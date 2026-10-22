use anyhow::Context;
use chrono::{DateTime, Utc};
use futures::{StreamExt, TryFutureExt};
use megaphone::client::model::StreamSpec;
use megaphone::client::MegaphoneClient;
use megaphone::dto::agent::OutcomeStatus;
use megaphone::dto::channel::ChannelCreateReqDto;
use megaphone::model::constants::protocols::HTTP_STREAM_NDJSON_V1;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;
use tempfile::TempDir;
use testcontainers::ContainerAsync;
use tokio::task::JoinHandle;

use crate::client::MegaphoneRestClient;
use crate::kubernetes::cluster::prepare_cluster;
use crate::testcontainers_ext::k3s;
use crate::testcontainers_ext::k3s::K3s;

mod client;
mod docker;
mod kubernetes;
mod testcontainers_ext;

static AIRGAP_DIR: OnceLock<TempDir> = OnceLock::new();
static K3S_CONF_DIR: OnceLock<TempDir> = OnceLock::new();
static CONTAINER: OnceLock<ContainerAsync<K3s>> = OnceLock::new();
static CLEANUP_REGISTERED: OnceLock<bool> = OnceLock::new();

extern "C" {
    fn atexit(callback: extern "C" fn()) -> std::os::raw::c_int;
}

/// Statics are never dropped, remove the shared cluster and temp dirs when the test process exits.
/// Must not panic: a panic escaping an `extern "C"` function aborts the process.
extern "C" fn cleanup_cluster() {
    if std::env::var("TESTCONTAINERS_COMMAND").is_ok_and(|cmd| cmd == "keep") {
        return;
    }
    let mut stderr = std::io::stderr();
    if let Some(container) = CONTAINER.get() {
        let out = std::process::Command::new("docker")
            .args(["rm", "--force", "--volumes", container.id()])
            .output();
        match out {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let _ = writeln!(
                    stderr,
                    "Error removing k3s container {} - {} {}",
                    container.id(),
                    out.status,
                    String::from_utf8_lossy(&out.stderr)
                );
                // the container still mounts the temp dirs, keep them
                return;
            }
            Err(err) => {
                let _ = writeln!(
                    stderr,
                    "Error removing k3s container {} - {err}",
                    container.id()
                );
                return;
            }
        }
    }
    for dir in [&AIRGAP_DIR, &K3S_CONF_DIR]
        .into_iter()
        .filter_map(OnceLock::get)
    {
        if let Err(err) = std::fs::remove_dir_all(dir.path()) {
            let _ = writeln!(stderr, "Error removing temp dir {:?} - {err}", dir.path());
        }
    }
}

fn temp_dir(cell: &'static OnceLock<TempDir>) -> anyhow::Result<&'static Path> {
    if cell.get().is_none() {
        let _ = cell.set(tempfile::tempdir().context("Error creating temp dir")?);
    }
    cell.get()
        .map(TempDir::path)
        .ok_or_else(|| anyhow::anyhow!("Oncelock not valorized"))
}

async fn get_container() -> anyhow::Result<&'static ContainerAsync<K3s>> {
    let cleanup_registered =
        *CLEANUP_REGISTERED.get_or_init(|| unsafe { atexit(cleanup_cluster) } == 0);
    if !cleanup_registered {
        anyhow::bail!("Cannot register cluster cleanup");
    }
    let result = if let Some(container) = CONTAINER.get() {
        container
    } else {
        let container = prepare_cluster(temp_dir(&AIRGAP_DIR)?, temp_dir(&K3S_CONF_DIR)?).await?;
        CONTAINER
            .set(container)
            .map_err(|_| anyhow::anyhow!("Cannot set oncelock"))?;
        let container = CONTAINER
            .get()
            .ok_or_else(|| anyhow::anyhow!("Oncelock not valorized"))?;
        container
    };
    Ok(result)
}

#[tokio::test]
#[serial_test::serial]
async fn channel_create() {
    let container = get_container()
        .await
        .expect("Error creating megaphone cluster");

    let client = MegaphoneRestClient::new(
        "localhost",
        container
            .get_host_port_ipv4(k3s::TRAEFIK_HTTP)
            .await
            .expect("Error retrieving traefik port"),
    );
    let res = client
        .create(&ChannelCreateReqDto {
            protocols: vec![String::from(HTTP_STREAM_NDJSON_V1)],
        })
        .await
        .expect("Error during new channel creation");

    assert!(res.protocols.contains(&String::from(HTTP_STREAM_NDJSON_V1)));
    assert!(!res.producer_address.is_empty());
    assert!(!res.consumer_address.is_empty());
}

#[tokio::test]
#[serial_test::serial]
async fn channel_write() {
    let container = get_container()
        .await
        .expect("Error creating megaphone cluster");

    let client = MegaphoneRestClient::new(
        "localhost",
        container
            .get_host_port_ipv4(k3s::TRAEFIK_HTTP)
            .await
            .expect("Error retrieving traefik port"),
    );
    let create_res = client
        .create(&ChannelCreateReqDto {
            protocols: vec![String::from(HTTP_STREAM_NDJSON_V1)],
        })
        .await
        .expect("Error during new channel creation");

    let write_res = client
        .write(
            &create_res.producer_address,
            "test",
            json!({"hello": "world"}),
        )
        .await
        .expect("Error writing to channel");

    assert!(matches!(write_res.status, OutcomeStatus::Ok))
}

#[tokio::test]
#[serial_test::serial]
async fn channel_read_write() {
    #[derive(Serialize, Deserialize)]
    struct TestMessage {
        message: String,
    }

    let container = get_container()
        .await
        .expect("Error creating megaphone cluster");

    let client = MegaphoneRestClient::new(
        "localhost",
        container
            .get_host_port_ipv4(k3s::TRAEFIK_HTTP)
            .await
            .expect("Error retrieving traefik port"),
    );
    let create_res = client
        .create(&ChannelCreateReqDto {
            protocols: vec![String::from(HTTP_STREAM_NDJSON_V1)],
        })
        .await
        .expect("Error during new channel creation");

    let write_res = client
        .write(
            &create_res.producer_address,
            "test",
            &TestMessage {
                message: String::from("Hello world"),
            },
        )
        .await
        .expect("Error writing to channel");

    assert!(matches!(write_res.status, OutcomeStatus::Ok));

    let read_res = client
        .read(&create_res.consumer_address)
        .await
        .expect("Error reading from channel");

    assert_eq!(String::from("test"), read_res.stream_id);
    let parsed_body =
        serde_json::from_value::<TestMessage>(read_res.body).expect("Cannot parse body");
    assert_eq!(String::from("Hello world"), parsed_body.message);
}

#[tokio::test]
#[serial_test::serial]
async fn channel_multi_read_write() {
    #[derive(Serialize, Deserialize)]
    struct TestMessage {
        timestamp: DateTime<Utc>,
        idx: i32,
    }

    let container = get_container()
        .await
        .expect("Error creating megaphone cluster");

    let client = MegaphoneRestClient::new(
        "localhost",
        container
            .get_host_port_ipv4(k3s::TRAEFIK_HTTP)
            .await
            .expect("Error retrieving traefik port"),
    );
    let create_res = client
        .create(&ChannelCreateReqDto {
            protocols: vec![String::from(HTTP_STREAM_NDJSON_V1)],
        })
        .await
        .expect("Error during new channel creation");

    let handle: JoinHandle<anyhow::Result<()>> = tokio::task::spawn(async move {
        for idx in 0..100 {
            let write_res = client
                .write(
                    &create_res.producer_address,
                    "test",
                    &TestMessage {
                        idx,
                        timestamp: Utc::now(),
                    },
                )
                .await?;

            assert!(matches!(write_res.status, OutcomeStatus::Ok));

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok(())
    });

    let mut read_client = MegaphoneClient::new(
        &format!(
            "http://localhost:{}/read",
            container
                .get_host_port_ipv4(k3s::TRAEFIK_HTTP)
                .await
                .expect("Error retrieving traefik port")
        ),
        100,
    );
    let mut stream = read_client
        .new_unbounded_stream(|_chan| {
            futures::future::ok::<_, anyhow::Error>(StreamSpec {
                channel: create_res.consumer_address.to_string(),
                streams: vec![String::from("test")],
            })
        })
        .await
        .expect("Error initializing read stream");

    let mut expected_idx = 0;
    while let Some(evt_res) = stream.next().await {
        let evt: TestMessage = evt_res.expect("Error in read event processing");
        assert_eq!(expected_idx, evt.idx);
        assert!(evt.timestamp + Duration::from_secs(1) > Utc::now());
        expected_idx += 1;
        if expected_idx >= 100 {
            break;
        }
    }

    handle
        .await
        .expect("Error joinhandle await")
        .expect("Error in producer result");
}

#[tokio::test]
#[serial_test::serial]
async fn channel_multi_read_write_multi_stream() {
    let container = get_container()
        .await
        .expect("Error creating megaphone cluster");

    let (producer_handle, even_consumer_handle, odd_consumer_handle) =
        verify_multi_stream_rw(&container, Duration::from_millis(500)).await;

    producer_handle
        .await
        .expect("Error joinhandle await")
        .expect("Error in producer result");
    odd_consumer_handle
        .await
        .expect("Error joinhandle await")
        .expect("Error in odd consumer result");
    even_consumer_handle
        .await
        .expect("Error joinhandle await")
        .expect("Error in even consumer result");
}

#[tokio::test]
#[serial_test::serial]
async fn multi_channel_multi_read_write_multi_stream() {
    let container = get_container()
        .await
        .expect("Error creating megaphone cluster");

    let channels_count = 10;
    let mut handles = Vec::with_capacity(channels_count * 3);

    for _ in 0..channels_count {
        let (producer_handle, even_consumer_handle, odd_consumer_handle) =
            verify_multi_stream_rw(&container, Duration::from_millis(0)).await;
        handles.push(producer_handle);
        handles.push(even_consumer_handle);
        handles.push(odd_consumer_handle);
    }

    futures::future::try_join_all(handles)
        .await
        .expect("Error joinhandle await")
        .into_iter()
        .collect::<anyhow::Result<Vec<()>>>()
        .expect("Error in spawned task");
}

async fn verify_multi_stream_rw(
    container: &ContainerAsync<K3s>,
    msg_delay: Duration,
) -> (
    JoinHandle<anyhow::Result<()>>,
    JoinHandle<anyhow::Result<()>>,
    JoinHandle<anyhow::Result<()>>,
) {
    #[derive(Serialize, Deserialize)]
    struct TestMessage {
        timestamp: DateTime<Utc>,
        idx: i32,
    }

    let client = MegaphoneRestClient::new(
        "localhost",
        container
            .get_host_port_ipv4(k3s::TRAEFIK_HTTP)
            .await
            .expect("Error retrieving traefik port"),
    );
    let create_res = client
        .create(&ChannelCreateReqDto {
            protocols: vec![String::from(HTTP_STREAM_NDJSON_V1)],
        })
        .await
        .expect("Error during new channel creation");

    let producer_handle: JoinHandle<anyhow::Result<()>> = tokio::task::spawn(async move {
        for idx in 0..100 {
            let stream_id = if idx % 2 == 0 { "even" } else { "odd" };

            let write_res = client
                .write(
                    &create_res.producer_address,
                    stream_id,
                    &TestMessage {
                        idx,
                        timestamp: Utc::now(),
                    },
                )
                .await?;

            assert!(matches!(write_res.status, OutcomeStatus::Ok));

            tokio::time::sleep(msg_delay).await;
        }
        Ok(())
    });

    let port = container
        .get_host_port_ipv4(k3s::TRAEFIK_HTTP)
        .await
        .expect("Error retrieving traefik port");
    let mut read_client = MegaphoneClient::new(&format!("http://localhost:{}/read", port), 100);
    let channel = create_res.consumer_address.to_string();
    let mut even_stream = read_client
        .new_unbounded_stream(move |_chan| {
            futures::future::ok::<_, anyhow::Error>(StreamSpec {
                channel: channel.clone(),
                streams: vec![String::from("even")],
            })
        })
        .await
        .expect("Error initializing read stream");
    let channel = create_res.consumer_address.to_string();
    let mut odd_stream = read_client
        .new_unbounded_stream(move |_chan| {
            futures::future::ok::<_, anyhow::Error>(StreamSpec {
                channel: channel.clone(),
                streams: vec![String::from("odd")],
            })
        })
        .await
        .expect("Error initializing read stream");

    let even_consumer_handle: JoinHandle<anyhow::Result<()>> = tokio::task::spawn(async move {
        let mut expected_even_idx = 0;
        while let Some(evt_res) = even_stream.next().await {
            let evt: TestMessage = evt_res.context("Error in read event processing")?;
            assert_eq!(expected_even_idx, evt.idx);
            assert!(evt.timestamp + Duration::from_secs(1) > Utc::now());
            expected_even_idx += 2;
            if expected_even_idx >= 100 {
                break;
            }
        }
        Ok(())
    });

    let odd_consumer_handle: JoinHandle<anyhow::Result<()>> = tokio::task::spawn(async move {
        let mut expected_odd_idx = 1;
        while let Some(evt_res) = odd_stream.next().await {
            let evt: TestMessage = evt_res.context("Error in read event processing")?;
            assert_eq!(expected_odd_idx, evt.idx);
            assert!(evt.timestamp + Duration::from_secs(1) > Utc::now());
            expected_odd_idx += 2;
            if expected_odd_idx >= 100 {
                break;
            }
        }
        Ok(())
    });
    (producer_handle, even_consumer_handle, odd_consumer_handle)
}
