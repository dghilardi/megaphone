mod testcontainers_ext;

use testcontainers::{clients, RunnableImage};
use crate::testcontainers_ext::k3s::K3s;

#[tokio::test]
async fn it_works() {
    let docker = clients::Cli::default();
    let k3s = RunnableImage::from(K3s::default())
        .with_privileged(true)
        .with_host_user_ns(true);
    let k3s_container = docker.run(k3s);
    k3s_container.start();
}