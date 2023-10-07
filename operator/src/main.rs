use std::sync::Arc;
use std::time::Duration;

use futures::stream::StreamExt;
use kube::{Api, Client};
use kube_derive::CustomResource;
use kube_runtime::Controller;
use kube_runtime::controller::Action;
use kube_runtime::watcher::Config;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
enum Error {}

/// A custom resource
#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(group = "d71.dev", version = "v1", kind = "Megaphone", namespaced)]
struct MegaphoneSpec {
    pub replicas: i32,
}

/// The reconciler that will be called when either object change
async fn reconcile(g: Arc<Megaphone>, _ctx: Arc<()>) -> Result<Action, Error> {
    // .. use api here to reconcile a child ConfigMap with ownerreferences
    // see configmapgen_controller example for full info
    Ok(Action::requeue(Duration::from_secs(300)))
}
/// an error handler that will be called when the reconciler fails with access to both the
/// object that caused the failure and the actual error
fn error_policy(obj: Arc<Megaphone>, _error: &Error, _ctx: Arc<()>) -> Action {
    Action::requeue(Duration::from_secs(60))
}

#[tokio::main]
async fn main() {
    let kubernetes_client = Client::try_default().await
        .expect("Expected a valid KUBECONFIG environment variable.");

    let context = Arc::new(()); // bad empty context - put client in here
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
