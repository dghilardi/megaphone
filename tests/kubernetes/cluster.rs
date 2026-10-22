use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::Context;
use futures::stream::StreamExt;
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet};
use k8s_openapi::api::core::v1::{ConfigMap, Pod, Service};
use k8s_openapi::api::networking::v1::Ingress;
use kube::api::PostParams;
use kube::runtime::{watcher, WatchStreamExt};
use kube::{Api, Client, ResourceExt};
use testcontainers::core::Mount;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};

use crate::kubernetes::client::get_kube_client;
use crate::testcontainers_ext::k3s;
use crate::testcontainers_ext::k3s::K3s;
use crate::{docker, kubernetes};

async fn wait_cluster_ready(client: &Client) -> anyhow::Result<()> {
    let mut stream = watcher(Api::<Pod>::all(client.clone()), Default::default())
        .applied_objects()
        .boxed();

    let min_ts = SystemTime::now() + Duration::from_secs(5);
    let deadline_ts = SystemTime::now() + Duration::from_secs(600);
    let mut last_state_update = SystemTime::now();
    let mut pods_status = HashMap::new();

    while let Some(evt) = tokio::time::timeout(Duration::from_secs(10), stream.next())
        .await
        .transpose()
    {
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
            && pods_status.values().all(|phase| {
                phase.eq_ignore_ascii_case("Running") || phase.eq_ignore_ascii_case("Succeeded")
            })
        {
            return Ok(());
        } else if pods_status
            .values()
            .any(|phase| phase.eq_ignore_ascii_case("Failed"))
        {
            anyhow::bail!("Cluster contains failed pods")
        } else if pods_status
            .values()
            .any(|phase| phase.eq_ignore_ascii_case("Unknown"))
        {
            anyhow::bail!("Cluster contains pods in unknown state")
        }
    }
    anyhow::bail!("Stream terminated before all pod running")
}

/// Pods may be running before traefik routes requests, wait until megaphone is reachable through the ingress
async fn wait_ingress_ready(container: &ContainerAsync<K3s>) -> anyhow::Result<()> {
    let port = container
        .get_host_port_ipv4(k3s::TRAEFIK_HTTP)
        .await
        .context("Error retrieving traefik port")?;
    let deadline_ts = SystemTime::now() + Duration::from_secs(300);
    loop {
        let res = reqwest::Client::new()
            .post(format!("http://localhost:{port}/create"))
            .send()
            .await;
        match res {
            Ok(res) if res.status().is_success() => return Ok(()),
            Ok(res) => log::debug!("Ingress not ready - status {}", res.status()),
            Err(err) => log::debug!("Ingress not ready - {err}"),
        }
        if deadline_ts < SystemTime::now() {
            anyhow::bail!("Deadline reached waiting for ingress");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

pub async fn prepare_cluster(
    airgap_dir: &Path,
    conf_dir: &Path,
) -> anyhow::Result<ContainerAsync<K3s>> {
    docker::builder::build_images(airgap_dir)?;

    let k3s = K3s
        .with_env_var("K3S_KUBECONFIG_MODE", "644")
        .with_privileged(true)
        .with_userns_mode("host")
        .with_startup_timeout(Duration::from_secs(600))
        .with_mount(Mount::bind_mount(
            airgap_dir.to_str().unwrap_or_default(),
            "/var/lib/rancher/k3s/agent/images/",
        ))
        .with_mount(Mount::bind_mount(
            conf_dir.to_str().unwrap_or_default(),
            "/etc/rancher/k3s/",
        ));
    let k3s_container = k3s.start().await.context("Error starting k3s container")?;

    let client = get_kube_client(&k3s_container, conf_dir)
        .await
        .context("Error extracting client")?;

    let configmap_api = Api::<ConfigMap>::default_namespaced(client.clone());
    configmap_api
        .create(
            &PostParams::default(),
            &kubernetes::resources::nginx::nginx_configmap(),
        )
        .await
        .context("Error creating nginx configmap")?;

    let service_api = Api::<Service>::default_namespaced(client.clone());
    service_api
        .create(
            &PostParams::default(),
            &kubernetes::resources::nginx::nginx_svc(),
        )
        .await
        .context("Error creating nginx service")?;

    service_api
        .create(
            &PostParams::default(),
            &kubernetes::resources::megaphone::megaphone_svc(),
        )
        .await
        .context("Error creating megaphone service")?;

    service_api
        .create(
            &PostParams::default(),
            &kubernetes::resources::megaphone::megaphone_headless_svc(),
        )
        .await
        .context("Error creating megaphone headless service")?;

    let stateful_set_api = Api::<StatefulSet>::default_namespaced(client.clone());
    stateful_set_api
        .create(
            &PostParams::default(),
            &kubernetes::resources::megaphone::megaphone_sts(2),
        )
        .await
        .context("Error applying megaphone statefulset")?;

    let deployment_api = Api::<Deployment>::default_namespaced(client.clone());
    deployment_api
        .create(
            &PostParams::default(),
            &kubernetes::resources::nginx::nginx_deployment(),
        )
        .await
        .context("Error applying nginx deployment")?;

    let ingress_api = Api::<Ingress>::default_namespaced(client.clone());
    ingress_api
        .create(
            &PostParams::default(),
            &kubernetes::resources::ingress::ingress(),
        )
        .await
        .context("Error creating ingress")?;

    wait_cluster_ready(&client)
        .await
        .context("Error awaiting cluster readiness")?;

    wait_ingress_ready(&k3s_container)
        .await
        .context("Error awaiting ingress readiness")?;

    Ok(k3s_container)
}
