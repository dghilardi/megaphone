use std::sync::Arc;

use anyhow::Result;
use k8s_openapi::{api::core::v1::{Container, Pod, PodSpec}, apimachinery::pkg::apis::meta::v1::OwnerReference};
use k8s_openapi::api::core::v1::{EnvVar, EnvVarSource, ObjectFieldSelector, Service, ServicePort, ServiceSpec};
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

pub struct MegaphonePod {
    name: String,
    virtual_agent_ids: Vec<String>,
    spec: Pod,
}

fn create_pod(resource_name: &str, idx: usize, namespace: &str, oref: OwnerReference, spec: &MegaphoneSpec) -> MegaphonePod {
    let virtual_agent_ids = (0..spec.virtual_agents_per_node)
        .into_iter()
        .map(|virtual_agent_idx| build_vagent_id(idx, virtual_agent_idx))
        .collect::<Vec<_>>();

    let env_vars = virtual_agent_ids.iter()
        .map(|virtual_agent_id| EnvVar {
            name: format!("megaphone_agent.virtual.{virtual_agent_id}"),
            value: Some(String::from("MASTER")),
            value_from: None,
        })
        .collect::<Vec<_>>();

    let pod_labels = virtual_agent_ids.iter()
        .into_iter()
        .flat_map(|virtual_agent_id| [
            (format!("megaphone-{virtual_agent_id}-write"), String::from("ON")),
            (format!("megaphone-{virtual_agent_id}-read"), String::from("ON")),
        ])
        .chain([(String::from("megaphone-cluster"), String::from(resource_name))])
        .collect();

    let pod_name = format!("megaphone-pod-{resource_name}-{idx}");

    MegaphonePod {
        name: pod_name.to_owned(),
        virtual_agent_ids,
        spec: Pod {
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
        },
    }
}

struct MegaphoneService {
    name: String,
    spec: Service,
}

fn create_service(cluster_name: &str, virtual_agent_id: &str, capability: &str, namespace: &str, oref: OwnerReference) -> MegaphoneService {
    let svc_name = format!("svc-{cluster_name}-{virtual_agent_id}-{capability}");
    MegaphoneService {
        name: svc_name.to_string(),
        spec: Service {
            metadata: ObjectMeta {
                name: Some(svc_name.to_owned()),
                namespace: Some(namespace.to_owned()),
                owner_references: Some(vec![oref]),
                labels: Some([
                    (String::from("svc-megaphone-cluster"), cluster_name.to_owned())
                ].into_iter().collect()),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                ports: Some(vec![ServicePort {
                    name: Some(String::from("http")),
                    port: 3000,
                    ..Default::default()
                }]),
                selector: Some([
                    (String::from("megaphone-cluster"), cluster_name.to_owned()),
                    (format!("megaphone-{virtual_agent_id}-{capability}"), String::from("ON")),
                ].into_iter().collect()),
                ..Default::default()
            }),
            status: None,
        }
    }
}

pub async fn reconcile(megaphone: Arc<Megaphone>, ctx: Arc<ContextData>) -> Result<Action, Error> {
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
    let services: Api<Service> = Api::namespaced(client.clone(), namespace);
    let workloads: Api<Megaphone> = Api::namespaced(client.clone(), namespace);

    let current_workloads = megaphone
        .status
        .clone()
        .unwrap_or_else(|| MegaphoneStatus::default())
        .pods
        .len();
    if current_workloads < megaphone.spec.replicas {
        let mut new_pods = Vec::<String>::new();
        let mut new_services = Vec::<String>::new();
        for pod_idx in current_workloads..megaphone.spec.replicas {
            let megaphone_pod = create_pod(&name, pod_idx, &namespace, oref.clone(), &megaphone.spec);
            let megaphone_services = megaphone_pod.virtual_agent_ids.iter()
                .flat_map(|virtual_agent_id| [
                    create_service(&name, virtual_agent_id, "read", &namespace, oref.clone()),
                    create_service(&name, virtual_agent_id, "write", &namespace, oref.clone()),
                ])
                .collect::<Vec<_>>();

            let res = pods
                .patch(
                    &megaphone_pod.name,
                    &PatchParams::apply("workload-Controller"),
                    &Patch::Apply(megaphone_pod.spec.clone()),
                )
                .await
                .map_err(Error::PodCreationFailed);

            match res {
                Ok(_) => new_pods.push(megaphone_pod.name),
                Err(e) => println!("Pod Creation Failed {:?}", e),
            }

            for megaphone_svc in megaphone_services {
                let res = services.patch(
                    &megaphone_svc.name,
                    &PatchParams::apply("workload-Controller"),
                    &Patch::Apply(megaphone_svc.spec.clone()),
                ).await;

                match res {
                    Ok(_) => new_services.push(megaphone_svc.name),
                    Err(e) => println!("Service Creation Failed {:?}", e),
                }
            }
        }
        let update_status = json!({
            "status": MegaphoneStatus { pods: new_pods, services: new_services }
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