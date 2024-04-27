use anyhow::Context;
use kube::{Client, Config};
use kube::config::{Kubeconfig, KubeConfigOptions};
use testcontainers::Container;
use testcontainers::core::ExecCommand;
use crate::testcontainers_ext::k3s;
use crate::testcontainers_ext::k3s::K3s;

pub async fn get_kube_client(container: &Container<K3s>) -> anyhow::Result<Client> {
    let out = container.exec(ExecCommand::new(vec![String::from("cat"), String::from("/etc/rancher/k3s/k3s.yaml")]));

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

pub async fn print_images(container: &Container<K3s>) -> anyhow::Result<()> {
    let out = container.exec(ExecCommand { cmd: String::from("crictl images"), ready_conditions: vec![] });

    let out_str = String::from_utf8(out.stdout)
        .context("Error parsing stdout to string")?;

    println!("{}", out_str);

    Ok(())
}