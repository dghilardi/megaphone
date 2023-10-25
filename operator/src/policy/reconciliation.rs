use std::sync::Arc;

use anyhow::Result;
use k8s_openapi::{api::core::v1::{Container, Pod, PodSpec}, apimachinery::pkg::apis::meta::v1::OwnerReference};
use k8s_openapi::api::core::v1::{EnvVar, EnvVarSource, ObjectFieldSelector};
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams, Resource},
    runtime::{
        controller::Action,
        finalizer::{Event as Finalizer, finalizer},
    },
};
use kube::api::DeleteParams;
use kube::error::ErrorResponse;
use serde_json::json;
use tokio::time::Duration;

use crate::model::context::ContextData;
use crate::model::error::Error;
use crate::model::spec::{Megaphone, MegaphoneSpec, MegaphoneStatus};

pub static WORKLOAD_FINALIZER: &str = "megaphone.d71.dev";

fn build_vagent_id(node_idx: usize, vagent_idx: usize) -> String {
    let scrambling_key = "MEGAPHONE".as_bytes();
    let scrambled_id = [].into_iter()
        .chain((node_idx as u32).to_be_bytes())
        .chain((vagent_idx as u32).to_be_bytes())
        .enumerate()
        .map(|(idx, b)| scrambling_key[idx % scrambling_key.len()] ^ b)
        .collect::<Vec<_>>();

    hex::encode(&scrambled_id)
}

fn create_pod(resource_name: &str, idx: usize, namespace: &str, oref: OwnerReference, spec: &MegaphoneSpec) -> (String, Pod) {
    let env_vars = (0..spec.virtual_agents_per_node)
        .into_iter()
        .map(|virtual_agent_idx| EnvVar {
            name: format!("megaphone_agent.virtual.{}", build_vagent_id(idx, virtual_agent_idx)),
            value: Some(String::from("MASTER")),
            value_from: None,
        })
        .collect::<Vec<_>>();

    let pod_labels = (0..spec.virtual_agents_per_node)
        .into_iter()
        .flat_map(|virtual_agent_idx| [
            (format!("megaphone-{}-write", build_vagent_id(idx, virtual_agent_idx)), String::from("ON")),
            (format!("megaphone-{}-read",  build_vagent_id(idx, virtual_agent_idx)), String::from("ON")),
        ])
        .chain([(String::from("megaphone-cluster"), String::from(resource_name))])
        .collect();

    let pod_name = format!("megaphone-pod-{resource_name}-{idx}");

    (pod_name.to_owned(), Pod {
        metadata: ObjectMeta {
            name: Some(pod_name.to_owned()),
            namespace: Some(namespace.to_owned()),
            owner_references: Some(vec![oref]),
            labels: Some(pod_labels),
            ..Default::default()
        },
        spec: Some(PodSpec {
            containers: vec![Container {
                name: pod_name.to_owned(),
                image: Some(String::from(&spec.image)),
                resources: spec.resources.clone()
                    .map(From::from),
                env: Some(env_vars),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

pub async fn reconcile(megaphone: Arc<Megaphone>, ctx: Arc<ContextData>) -> Result<Action, Error> {
    match determine_action(&megaphone) {
        MegaphoneAction::Create => create(megaphone, ctx).await,
        MegaphoneAction::Delete => delete(megaphone, ctx).await,
        // The resource is already in desired state, do nothing and re-check after 300 seconds
        MegaphoneAction::NoOp => Ok(Action::requeue(Duration::from_secs(300))),
    }
}

pub async fn delete(megaphone: Arc<Megaphone>, ctx: Arc<ContextData>) -> Result<Action, Error> {
    let client = ctx.client.clone();

    let namespace = megaphone
        .metadata
        .namespace
        .as_ref()
        .ok_or_else(|| Error::MissingObjectKey(".metadata.namespace"))
        .unwrap();

    if let Some(status) = megaphone.status.as_ref() {
        let api: Api<Pod> = Api::namespaced(client, namespace);
        for name in &status.pods {
            let delete_out = api.delete(name, &DeleteParams::default()).await;
            match delete_out {
                Ok(_) => {}
                Err(kube::error::Error::Api(ErrorResponse { reason, .. })) if reason.eq("NotFound") => {
                    eprintln!("Resource not found - {name}");
                },
                Err(err) => return Err(Error::PodDeletionFailed(err)),
            }
        }
    }
    Ok(Action::requeue(Duration::from_secs(300)))
}
pub async fn create(megaphone: Arc<Megaphone>, ctx: Arc<ContextData>) -> Result<Action, Error> {
    let client = &ctx.client;

    let namespace = megaphone
        .metadata
        .namespace
        .as_ref()
        .ok_or_else(|| Error::MissingObjectKey(".metadata.namespace"))
        .unwrap();

    let name = megaphone
        .metadata
        .name
        .as_ref()
        .ok_or_else(|| Error::MissingObjectKey(".metadata.names"))
        .unwrap();

    let oref = megaphone.controller_owner_ref(&()).unwrap();

    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let workloads: Api<Megaphone> = Api::namespaced(client.clone(), namespace);

    let current_workloads = megaphone
        .status
        .clone()
        .unwrap_or_else(|| MegaphoneStatus::default())
        .pods
        .len();
    if current_workloads < megaphone.spec.replicas {
        let mut new_pods = Vec::<String>::new();
        for pod_idx in current_workloads..megaphone.spec.replicas {
            let (pod_name, pod) = create_pod(&name, pod_idx, &namespace, oref.clone(), &megaphone.spec);
            let res = pods
                .patch(
                    &pod_name,
                    &PatchParams::apply("workload-Controller"),
                    &Patch::Apply(pod.clone()),
                )
                .await
                .map_err(Error::PodCreationFailed);

            println!("{:?}", res);

            match res {
                Ok(_) => new_pods.push(pod_name),
                Err(e) => println!("Pod Creation Failed {:?}", e),
            }
        }
        let update_status = json!({
            "status": MegaphoneStatus { pods: new_pods }
        });
        let res = workloads
            .patch_status(name, &PatchParams::default(), &Patch::Merge(&update_status))
            .await;

        if let Err(err) = res {
            println!("Pod Creation Failed {:?}", err);
        }
    }

    finalizer(&workloads, WORKLOAD_FINALIZER, megaphone, |event| async {
        match event {
            Finalizer::Cleanup(workload) => {
                println!("Finalizing Workload: {} ...!", workload.meta().name.clone().unwrap());
                Ok(Action::await_change())
            }
            _ => Ok(Action::await_change()),
        }
    }).await.map_err(|e| Error::FinalizerError(Box::new(e)))?;
    Ok(Action::requeue(Duration::from_secs(300)))
}

enum MegaphoneAction {
    /// Create the subresources, this includes spawning `n` pods with Echo service
    Create,
    /// Delete all subresources created in the `Create` phase
    Delete,
    /// This `Echo` resource is in desired state and requires no actions to be taken
    NoOp,
}
fn determine_action(megaphone: &Megaphone) -> MegaphoneAction {
    return if megaphone.meta().deletion_timestamp.is_some() {
        MegaphoneAction::Delete
    } else if megaphone
        .meta()
        .finalizers
        .as_ref()
        .map_or(true, |finalizers| finalizers.is_empty())
    {
        MegaphoneAction::Create
    } else {
        MegaphoneAction::NoOp
    };
}