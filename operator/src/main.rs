use std::sync::Arc;

use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::Api,
    Client,
    runtime::controller::Controller,
};
use kube::runtime::watcher::Config;

use crate::model::context::ContextData;
use crate::model::spec::Megaphone;
use crate::policy::error::error_policy;
use crate::policy::reconciliation::reconcile;

mod model;

mod policy;
mod service;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let kubernetes_client = Client::try_default().await?;
    
    let context = Arc::new(ContextData {
        client: kubernetes_client.clone(),
    });

    let crd_api = Api::<Megaphone>::all(kubernetes_client.clone());
    let podapi = Api::<Pod>::all(kubernetes_client.clone());
    
    Controller::new(crd_api.clone(), Config::default())
    .owns(podapi, Config::default())
    .shutdown_on_signal()
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


    Ok(())
}
