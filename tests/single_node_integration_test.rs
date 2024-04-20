mod testcontainers_ext;
mod kubernetes;
mod docker;

use lazy_static::lazy_static;
use testcontainers::clients::Cli;
use crate::kubernetes::cluster::prepare_cluster;

lazy_static! {
    static ref AIRGAP_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating airgap temp dir");
    static ref K3S_CONF_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating conf temp dir");
}

#[tokio::test]
async fn it_works() {
    let docker = Cli::default();
    prepare_cluster(&docker, AIRGAP_DIR.path())
        .await
        .expect("Error creating megaphone cluster");
}
