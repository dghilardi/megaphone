use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::Image;

const NAME: &str = "rancher/k3s";
const TAG: &str = "v1.28.8-k3s1";
pub const TRAEFIK_HTTP: u16 = 80;
pub const KUBE_SECURE_PORT: u16 = 6443;
pub const RANCHER_WEBHOOK_PORT: u16 = 8443;

#[derive(Debug, Default, Clone)]
pub struct K3s;

impl Image for K3s {
    fn name(&self) -> &str {
        NAME
    }

    fn tag(&self) -> &str {
        TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stderr(
            "Node controller sync successful",
        )]
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<std::borrow::Cow<'_, str>>> {
        ["server", "--snapshotter=native"]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[
            ContainerPort::Tcp(KUBE_SECURE_PORT),
            ContainerPort::Tcp(RANCHER_WEBHOOK_PORT),
            ContainerPort::Tcp(TRAEFIK_HTTP),
        ]
    }
}
