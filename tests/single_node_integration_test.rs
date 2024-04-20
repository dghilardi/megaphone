mod testcontainers_ext;
mod kubernetes;
mod docker;

use std::fs::read_to_string;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use anyhow::Context;
use k8s_openapi::api;
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet, StatefulSetSpec};
use k8s_openapi::api::core::v1::{ConfigMap, ContainerPort, EnvVar, EnvVarSource, ObjectFieldSelector, PodSpec, PodTemplateSpec, ResourceRequirements, Service};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use kube::{Api, Client, Config};
use kube::api::{PatchParams, PostParams};
use kube::client::ClientBuilder;
use kube::config::{Kubeconfig, KubeConfigOptions};
use lazy_static::lazy_static;
use serde_json::json;
use testcontainers::{clients, Container, GenericImage, RunnableImage};
use testcontainers::core::ExecCommand;
use crate::testcontainers_ext::k3s;
use crate::testcontainers_ext::k3s::K3s;

lazy_static! {
    static ref AIRGAP_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating airgap temp dir");
    static ref K3S_CONF_DIR: tempfile::TempDir = tempfile::tempdir().expect("Error creating conf temp dir");
}



async fn get_kube_client(container: &Container<'_, K3s>) -> anyhow::Result<Client> {
    let out = container.exec(ExecCommand { cmd: String::from("cat /etc/rancher/k3s/k3s.yaml"), ready_conditions: vec![] });

    let conf_yaml = String::from_utf8(out.stdout)
        .context("Error parsing stdout to string")?;

    let mut config = Kubeconfig::from_yaml(&conf_yaml)
        .context("Error loading kube config")?;

    config.clusters.iter_mut()
        .for_each(|cluster| {
            if let Some(server) = cluster.cluster.as_mut().and_then(|c| c.server.as_mut()) {
                *server = format!("https://127.0.0.1:{}", container.get_host_port_ipv4(k3s::KUBE_SECURE_PORT))
            }
        });

    let client_config = Config::from_custom_kubeconfig(config, &KubeConfigOptions::default())
        .await
        .context("Error building client config")?;

    let client = Client::try_from(client_config)
        .context("Error building client")?;

    Ok(client)
}

#[tokio::test]
async fn it_works() {
    docker::builder::build_images(AIRGAP_DIR.path().join("k3s-airgap-images-amd64.tar"));
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
    let configmap_api = Api::<ConfigMap>::default_namespaced(client.clone());
    configmap_api.create(&PostParams::default(), &kubernetes::resources::nginx::nginx_configmap())
        .await
        .expect("Error creating nginx configmap");

    let service_api = Api::<Service>::default_namespaced(client.clone());
    service_api.create(&PostParams::default(), &kubernetes::resources::megaphone::megaphone_svc())
        .await
        .expect("Error creating megaphone service");

    service_api.create(&PostParams::default(), &kubernetes::resources::megaphone::megaphone_headless_svc())
        .await
        .expect("Error creating megaphone headless service");

    let stateful_set_api = Api::<StatefulSet>::default_namespaced(client.clone());
    stateful_set_api.create(&PostParams::default(), &kubernetes::resources::megaphone::megaphone_sts(2))
        .await
        .expect("Error applying megaphone statefulset");

    let deployment_api = Api::<Deployment>::default_namespaced(client.clone());
    deployment_api.create(&PostParams::default(), &kubernetes::resources::nginx::nginx_deployment())
        .await
        .expect("Error applying nginx deployment");

    tokio::time::sleep(Duration::from_secs(300)).await;

}
