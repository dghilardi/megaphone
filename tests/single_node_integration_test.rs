mod testcontainers_ext;

use std::process::Command;
use lazy_static::lazy_static;
use testcontainers::{clients, GenericImage, RunnableImage};
use crate::testcontainers_ext::k3s::K3s;

const IMAGE_NAME: &str = "registry.d71.dev/megaphone:latest";

lazy_static! {
    static ref AIRGAP_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating temp dir");
}

fn build_images() {
    let megaphone_path = AIRGAP_DIR.path().join("megaphone.tgz");
    if !megaphone_path.is_file() {
        Command::new("docker")
            .arg("build")
            .arg("-f")
            .arg("dockerfile/Dockerfile")
            .arg(".")
            .arg("-t")
            .arg(IMAGE_NAME)
            .output()
            .expect("Error building megaphone image");

        Command::new("docker")
            .arg("save")
            .arg(IMAGE_NAME)
            .arg("-o")
            .arg(megaphone_path)
            .output()
            .expect("Error saving megaphone image");
    }
}

#[tokio::test]
async fn it_works() {
    build_images();
    let docker = clients::Cli::default();

    let k3s = RunnableImage::from(K3s::default())
        .with_privileged(true)
        .with_host_user_ns(true)
        .with_volume((AIRGAP_DIR.path().to_str().unwrap_or_default(), "/var/lib/rancher/k3s/agent/images/"));
    let k3s_container = docker.run(k3s);
    k3s_container.start();
}