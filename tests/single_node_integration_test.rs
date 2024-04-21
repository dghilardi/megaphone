use lazy_static::lazy_static;
use megaphone::dto::agent::OutcomeStatus;
use megaphone::dto::channel::ChannelCreateReqDto;
use megaphone::model::constants::protocols::HTTP_STREAM_NDJSON_V1;
use serde_json::json;
use testcontainers::clients::Cli;

use crate::client::MegaphoneRestClient;
use crate::kubernetes::cluster::prepare_cluster;
use crate::testcontainers_ext::k3s;

mod testcontainers_ext;
mod kubernetes;
mod docker;
mod client;

lazy_static! {
    static ref AIRGAP_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating airgap temp dir");
    static ref K3S_CONF_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating conf temp dir");
}

#[tokio::test]
#[serial_test::serial]
async fn channel_create() {
    let docker = Cli::default();
    let container = prepare_cluster(&docker, AIRGAP_DIR.path())
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
    let docker = Cli::default();
    let container = prepare_cluster(&docker, AIRGAP_DIR.path())
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
