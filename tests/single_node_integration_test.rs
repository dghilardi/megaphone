mod testcontainers_ext;

use std::fs::read_to_string;
use std::process::Command;
use anyhow::Context;
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet};
use kube::{Api, Client, Config};
use kube::api::{PatchParams, PostParams};
use kube::client::ClientBuilder;
use kube::config::{Kubeconfig, KubeConfigOptions};
use lazy_static::lazy_static;
use serde_json::json;
use testcontainers::{clients, Container, GenericImage, RunnableImage};
use testcontainers::core::ExecCommand;
use crate::testcontainers_ext::k3s::K3s;

const IMAGE_NAME: &str = "registry.d71.dev/megaphone:latest";

lazy_static! {
    static ref AIRGAP_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating airgap temp dir");
    static ref K3S_CONF_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating conf temp dir");
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

async fn get_kube_client(container: &Container<'_, K3s>) -> anyhow::Result<Client> {
    let out = container.exec(ExecCommand { cmd: String::from("cat /etc/rancher/k3s/k3s.yaml"), ready_conditions: vec![] });

    let conf_yaml = String::from_utf8(out.stdout)
        .context("Error parsing stdout to string")?;

    println!("{conf_yaml}");
    let config = Kubeconfig::from_yaml(&conf_yaml)
        .context("Error loading kube config")?;

    let client_config = Config::from_custom_kubeconfig(config, &KubeConfigOptions::default())
        .await
        .context("Error building client config")?;

    let client = Client::try_from(client_config)
        .context("Error building client")?;

    Ok(client)
}

#[tokio::test]
async fn it_works() {
    build_images();
    let docker = clients::Cli::default();

    let k3s = RunnableImage::from(K3s::default())
        .with_privileged(true)
        .with_host_user_ns(true)
        .with_volume((AIRGAP_DIR.path().to_str().unwrap_or_default(), "/var/lib/rancher/k3s/agent/images/"))
        .with_volume((K3S_CONF_DIR.path().to_str().unwrap_or_default(), "/etc/rancher/k3s"))
        ;
    let k3s_container = docker.run(k3s);
    k3s_container.start();

    let client = get_kube_client(&k3s_container).await.expect("Error extracting client");
    let deployment_api = Api::<StatefulSet>::default_namespaced(client.clone());
    deployment_api.create(&PostParams::default(), &StatefulSet {
        metadata: Default::default(),
        spec: None,
        status: None,
    }).await.expect("Error applying megaphone deployment");
}