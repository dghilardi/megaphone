use std::sync::OnceLock;
use lazy_static::lazy_static;
use megaphone::dto::agent::OutcomeStatus;
use megaphone::dto::channel::ChannelCreateReqDto;
use megaphone::model::constants::protocols::HTTP_STREAM_NDJSON_V1;
use serde_json::json;
use testcontainers::clients::Cli;
use testcontainers::Container;

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

    static ref DOCKER: Cli = Cli::default();
}
static CONTAINER: OnceLock<Container<'static, K3s>> = OnceLock::new();

async fn get_container() -> anyhow::Result<&'static Container<'static, K3s>> {
    let result = if let Some(container) = CONTAINER.get() {
        container
    } else {
        let container = prepare_cluster(&DOCKER, AIRGAP_DIR.path()).await?;
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
