use std::sync::OnceLock;
use std::time::Duration;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use megaphone::client::MegaphoneClient;
use megaphone::client::model::StreamSpec;
use megaphone::dto::agent::OutcomeStatus;
use megaphone::dto::channel::ChannelCreateReqDto;
use megaphone::model::constants::protocols::HTTP_STREAM_NDJSON_V1;
use serde::{Deserialize, Serialize};
use serde_json::de::Read;
use serde_json::json;
use testcontainers::Container;
use tokio::task::JoinHandle;
use futures::StreamExt;
use anyhow::Context;

use crate::client::MegaphoneRestClient;
use crate::kubernetes::cluster::prepare_cluster;
use crate::testcontainers_ext::k3s;
use crate::testcontainers_ext::k3s::K3s;

mod testcontainers_ext;
mod kubernetes;
mod docker;
mod client;

lazy_static! {
    static ref AIRGAP_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating airgap temp dir");
    static ref K3S_CONF_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating conf temp dir");
}
static CONTAINER: OnceLock<Container<K3s>> = OnceLock::new();

async fn get_container() -> anyhow::Result<&'static Container<K3s>> {
    let result = if let Some(container) = CONTAINER.get() {
        container
    } else {
        let container = prepare_cluster(AIRGAP_DIR.path()).await?;
        CONTAINER.set(container)
            .map_err(|_| anyhow::anyhow!("Cannot set oncelock"))?;
        let container = CONTAINER.get()
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

    let client = MegaphoneRestClient::new("localhost", container.get_host_port_ipv4(k3s::TRAEFIK_HTTP));
    let res = client.create(&ChannelCreateReqDto {
        protocols: vec![String::from(HTTP_STREAM_NDJSON_V1)],
    }).await.expect("Error during new channel creation");

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

    let client = MegaphoneRestClient::new("localhost", container.get_host_port_ipv4(k3s::TRAEFIK_HTTP));
    let create_res = client.create(&ChannelCreateReqDto {
        protocols: vec![String::from(HTTP_STREAM_NDJSON_V1)],
    }).await.expect("Error during new channel creation");

    let write_res = client.write(
        &create_res.producer_address,
        "test",
        json!({"hello": "world"})
    ).await.expect("Error writing to channel");

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

    let client = MegaphoneRestClient::new("localhost", container.get_host_port_ipv4(k3s::TRAEFIK_HTTP));
    let create_res = client.create(&ChannelCreateReqDto {
        protocols: vec![String::from(HTTP_STREAM_NDJSON_V1)],
    }).await.expect("Error during new channel creation");

    let write_res = client.write(
        &create_res.producer_address,
        "test",
        &TestMessage { message: String::from("Hello world") }
    ).await.expect("Error writing to channel");

    assert!(matches!(write_res.status, OutcomeStatus::Ok));

    let read_res = client.read(&create_res.consumer_address)
        .await
        .expect("Error reading from channel");

    assert_eq!(String::from("test"), read_res.stream_id);
    let parsed_body = serde_json::from_value::<TestMessage>(read_res.body).expect("Cannot parse body");
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

    let client = MegaphoneRestClient::new("localhost", container.get_host_port_ipv4(k3s::TRAEFIK_HTTP));
    let create_res = client.create(&ChannelCreateReqDto {
        protocols: vec![String::from(HTTP_STREAM_NDJSON_V1)],
    }).await.expect("Error during new channel creation");

    let handle: JoinHandle<anyhow::Result<()>> = tokio::task::spawn(async move {
        for idx in 0..100 {
            let write_res = client.write(
                &create_res.producer_address,
                "test",
                &TestMessage { idx, timestamp: Utc::now() }
            ).await?;

            assert!(matches!(write_res.status, OutcomeStatus::Ok));

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok(())
    });

    let mut read_client = MegaphoneClient::new(&format!("http://localhost:{}/read", container.get_host_port_ipv4(k3s::TRAEFIK_HTTP)), 100);
    let mut stream = read_client.new_unbounded_stream(|_chan| {
        futures::future::ok::<_, anyhow::Error>(StreamSpec { channel: create_res.consumer_address.to_string(), streams: vec![String::from("test")] })
    }).await.expect("Error initializing read stream");

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
