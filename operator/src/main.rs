use std::sync::Arc;

use futures::stream::StreamExt;
use kube::{Api, Client};
use kube_runtime::Controller;
use kube_runtime::watcher::Config;

use crate::model::context::ContextData;
use crate::policy::error::error_policy;
use crate::policy::reconciliation::reconcile;
use crate::spec::Megaphone;

mod spec;
mod policy;
mod model;

#[tokio::main]
async fn main() {
    let kubernetes_client = Client::try_default().await
        .expect("Expected a valid KUBECONFIG environment variable.");

    let context = Arc::new(ContextData {
        client: kubernetes_client.clone(),
    });

    let crd_api = Api::<Megaphone>::all(kubernetes_client.clone());

    // The controller comes from the `kube_runtime` crate and manages the reconciliation process.
    // It requires the following information:
    // - `kube::Api<T>` this controller "owns". In this case, `T = Echo`, as this controller owns the `Echo` resource,
    // - `kube::api::ListParams` to select the `Echo` resources with. Can be used for Echo filtering `Echo` resources before reconciliation,
    // - `reconcile` function with reconciliation logic to be called each time a resource of `Echo` kind is created/updated/deleted,
    // - `on_error` function to call whenever reconciliation fails.
    Controller::new(crd_api.clone(), Config::default())
        .run(reconcile, error_policy, context)
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(megaphone_resource) => {
                    println!("Reconciliation successful. Resource: {:?}", megaphone_resource);
                }
                Err(reconciliation_err) => {
                    eprintln!("Reconciliation error: {:?}", reconciliation_err)
                }
            }
        })
        .await;
}
