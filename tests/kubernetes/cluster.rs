use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::Context;
use futures::stream::StreamExt;
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet};
use k8s_openapi::api::core::v1::{ConfigMap, Pod, Service};
use k8s_openapi::api::networking::v1::Ingress;
use kube::{Api, Client, ResourceExt};
use kube::api::PostParams;
use kube::runtime::{watcher, WatchStreamExt};
use testcontainers::{Container, RunnableImage};
use testcontainers::clients::Cli;

use crate::{docker, kubernetes};
use crate::kubernetes::client::get_kube_client;
use crate::testcontainers_ext::k3s::K3s;

async fn wait_cluster_ready(client: &Client) -> anyhow::Result<()> {
    let mut stream = watcher(Api::<Pod>::all(client.clone()), Default::default())
        .applied_objects()
        .boxed();

    let min_ts = SystemTime::now() + Duration::from_secs(5);
    let deadline_ts = SystemTime::now() + Duration::from_secs(600);
    let mut last_state_update = SystemTime::now();
    let mut pods_status = HashMap::new();

    while let Some(evt) = tokio::time::timeout(Duration::from_secs(10), stream.next()).await.transpose() {
        match evt {
            Ok(Ok(evt)) => {
                let pod: Pod = evt;
                if let Some(phase) = pod.status.as_ref().and_then(|status| status.phase.clone()) {
                    pods_status.insert(pod.name_any(), phase);
                    last_state_update = SystemTime::now();
                }
            }
            Ok(Err(err)) => {
                anyhow::bail!("Received error during watch - {err}")
            }
            Err(_err) => {
                log::debug!("Timeout")
            }
        }

        if deadline_ts < SystemTime::now() {
            anyhow::bail!("Deadline reached - pods status: {pods_status:?}");
        } else if min_ts < SystemTime::now()
            && last_state_update + Duration::from_secs(10) < SystemTime::now()
            && pods_status.values().all(|phase| phase.eq_ignore_ascii_case("Running") || phase.eq_ignore_ascii_case("Succeeded")) {
            return Ok(())
        } else if pods_status.values().any(|phase| phase.eq_ignore_ascii_case("Failed")) {
            anyhow::bail!("Cluster contains failed pods")
        } else if pods_status.values().any(|phase| phase.eq_ignore_ascii_case("Unknown")) {
            anyhow::bail!("Cluster contains pods in unknown state")
        }
    }
    anyhow::bail!("Stream terminated before all pod running")
}

pub async fn prepare_cluster<'a>(docker: &'a Cli, airgap_dir: &Path) -> anyhow::Result<Container<'a, K3s>> {
    docker::builder::build_images(airgap_dir);

    let k3s = RunnableImage::from(K3s::default())
        .with_privileged(true)
        .with_host_user_ns(true)
        .with_volume((airgap_dir.to_str().unwrap_or_default(), "/var/lib/rancher/k3s/agent/images/"))
        ;
    let k3s_container = docker.run(k3s);
    k3s_container.start();

    let client = get_kube_client(&k3s_container).await.context("Error extracting client")?;
    let configmap_api = Api::<ConfigMap>::default_namespaced(client.clone());
    configmap_api.create(&PostParams::default(), &kubernetes::resources::nginx::nginx_configmap())
        .await
        .context("Error creating nginx configmap")?;

    let service_api = Api::<Service>::default_namespaced(client.clone());
    service_api.create(&PostParams::default(), &kubernetes::resources::nginx::nginx_svc())
        .await
        .context("Error creating nginx service")?;

    service_api.create(&PostParams::default(), &kubernetes::resources::megaphone::megaphone_svc())
        .await
        .context("Error creating megaphone service")?;

    service_api.create(&PostParams::default(), &kubernetes::resources::megaphone::megaphone_headless_svc())
        .await
        .context("Error creating megaphone headless service")?;

    let stateful_set_api = Api::<StatefulSet>::default_namespaced(client.clone());
    stateful_set_api.create(&PostParams::default(), &kubernetes::resources::megaphone::megaphone_sts(2))
        .await
        .context("Error applying megaphone statefulset")?;

    let deployment_api = Api::<Deployment>::default_namespaced(client.clone());
    deployment_api.create(&PostParams::default(), &kubernetes::resources::nginx::nginx_deployment())
        .await
        .context("Error applying nginx deployment")?;

    let ingress_api = Api::<Ingress>::default_namespaced(client.clone());
    ingress_api.create(&PostParams::default(), &kubernetes::resources::ingress::ingress())
        .await
        .context("Error creating ingress")?;

    wait_cluster_ready(&client)
        .await
        .context("Error awaiting cluster readiness")?;

    Ok(k3s_container)
}