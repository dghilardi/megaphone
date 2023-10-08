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
use serde_json::json;
use tokio::time::Duration;

use crate::model::context::ContextData;
use crate::model::error::Error;
use crate::model::spec::{Megaphone, MegaphoneSpec, MegaphoneStatus};

pub static WORKLOAD_FINALIZER: &str = "megaphone.d71.dev";

fn create_pod(name: &str, namespace: &str, oref: OwnerReference, spec: &MegaphoneSpec) -> Pod {
    Pod {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            owner_references: Some(vec![oref]),
            ..Default::default()
        },
        spec: Some(PodSpec {
            containers: vec![Container {
                name: name.to_owned(),
                image: Some(String::from(&spec.image)),
                resources: spec.resources.clone()
                    .map(From::from),
                env: Some(vec![
                    EnvVar {
                        name: String::from("megaphone_agent_name"),
                        value: None,
                        value_from: Some(EnvVarSource {
                            field_ref: Some(ObjectFieldSelector {
                                api_version: None,
                                field_path: String::from("metadata.name"),
                            }),
                            ..Default::default()
                        }),
                    }
                ]),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
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
    let workloads: Api<Megaphone> = Api::namespaced(client.clone(), namespace);

    let current_workloads = megaphone
        .status
        .clone()
        .unwrap_or_else(|| MegaphoneStatus::default())
        .pods
        .len();
    if current_workloads < megaphone.spec.replicas {
        let mut new_pods = Vec::<String>::new();
        for i in current_workloads..megaphone.spec.replicas {
            let mut pod_name = String::from("megaphone-pod-");
            pod_name.push_str(name);
            pod_name.push_str("-");
            pod_name.push_str(&i.to_string());
            let pod = create_pod(&pod_name, &namespace, oref.clone(), &megaphone.spec);
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