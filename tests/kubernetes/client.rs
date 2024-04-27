use std::path::Path;

use anyhow::Context;
use kube::{Client, Config};
use kube::config::{Kubeconfig, KubeConfigOptions};
use testcontainers::ContainerAsync;
use tokio::fs;

use crate::testcontainers_ext::k3s;
use crate::testcontainers_ext::k3s::K3s;

pub async fn get_kube_client(container: &ContainerAsync<K3s>, conf_dir: &Path) -> anyhow::Result<Client> {
    let source_dir = conf_dir.join("k3s.yaml");

    let conf_yaml = fs::read_to_string(&source_dir).await
        .context("Error reading k3s.yaml")?;

    let mut config = Kubeconfig::from_yaml(&conf_yaml)
        .context("Error loading kube config")?;

    let port = container.get_host_port_ipv4(k3s::KUBE_SECURE_PORT).await;
    config.clusters.iter_mut()
        .for_each(|cluster| {
            if let Some(server) = cluster.cluster.as_mut().and_then(|c| c.server.as_mut()) {
                *server = format!("https://127.0.0.1:{}", port)
            }
        });

    let client_config = Config::from_custom_kubeconfig(config, &KubeConfigOptions::default())
        .await
        .context("Error building client config")?;

    let client = Client::try_from(client_config)
        .context("Error building client")?;

    Ok(client)
}