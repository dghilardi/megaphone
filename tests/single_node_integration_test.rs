mod testcontainers_ext;
mod kubernetes;
mod docker;

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};
use anyhow::Context;
use k8s_openapi::api;
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet, StatefulSetSpec};
use k8s_openapi::api::core::v1::{ConfigMap, ContainerPort, EnvVar, EnvVarSource, ObjectFieldSelector, Pod, PodSpec, PodTemplateSpec, ResourceRequirements, Service};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use kube::{Api, Client, Config, ResourceExt};
use kube::api::{PatchParams, PostParams};
use kube::client::ClientBuilder;
use kube::config::{Kubeconfig, KubeConfigOptions};
use kube::runtime::{watcher, WatchStreamExt};
use kube::runtime::watcher::Event;
use lazy_static::lazy_static;
use serde_json::json;
use testcontainers::{clients, Container, GenericImage, RunnableImage};
use testcontainers::core::ExecCommand;
use crate::testcontainers_ext::k3s;
use crate::testcontainers_ext::k3s::K3s;
use futures::stream::StreamExt;
use k8s_openapi::api::networking::v1::Ingress;

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

async fn wait_cluster_ready(client: &Client) -> anyhow::Result<()> {
    let mut stream = watcher(Api::<Pod>::default_namespaced(client.clone()), Default::default())
        .applied_objects()
        .boxed();

    let min_ts = SystemTime::now() + Duration::from_secs(5);
    let mut pods_status = HashMap::new();

    while let Some(evt) = stream.next().await {
        match evt {
            Ok(evt) => {
                let pod: Pod = evt;
                if let Some(phase) = pod.status.as_ref().and_then(|status| status.phase.clone()) {
                    pods_status.insert(pod.name_any(), phase);
                }
                if min_ts < SystemTime::now()
                && pods_status.values().all(|phase| phase.eq_ignore_ascii_case("Running")) {
                    return Ok(())
                }
            }
            Err(err) => {
                anyhow::bail!("Received error during watch - {err}")
            }
        }
    }
    anyhow::bail!("Stream terminated before all pod running")
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
    service_api.create(&PostParams::default(), &kubernetes::resources::nginx::nginx_svc())
        .await
        .expect("Error creating nginx service");

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

    let ingress_api = Api::<Ingress>::default_namespaced(client.clone());
    ingress_api.create(&PostParams::default(), &kubernetes::resources::ingress::ingress())
        .await
        .expect("Error creating ingress");

    wait_cluster_ready(&client).await.expect("Error awaiting cluster readiness");
}
