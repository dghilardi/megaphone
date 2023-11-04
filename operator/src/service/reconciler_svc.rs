use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;
use k8s_openapi::{api::core::v1::{Container, Pod, PodSpec}, apimachinery::pkg::apis::meta::v1::OwnerReference};
use k8s_openapi::api::core::v1::{EnvVar, Service, ServicePort, ServiceSpec};
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams, Resource},
    runtime::{
        controller::Action,
        finalizer::{Event as Finalizer, finalizer},
    },
};
use kube::core::PartialObjectMetaExt;
use serde_json::json;
use tokio::time::Duration;

use megaphone::dto::agent::{VirtualAgentItemDto, VirtualAgentModeDto};

use crate::model::context::ContextData;
use crate::model::error::Error;
use crate::model::spec::{Megaphone, MegaphoneClusterStatus, MegaphoneStatus};
use crate::service::megactl_svc::MegactlService;

pub static WORKLOAD_FINALIZER: &str = "megaphone.d71.dev";

pub struct MegaphoneReconciler {
    megaphone: Arc<Megaphone>,
    ctx: Arc<ContextData>,
}

struct MegaphonePod {
    name: String,
    virtual_agent_ids: Vec<String>,
    spec: Pod,
}

struct MegaphoneService {
    name: String,
    spec: Service,
}

impl MegaphoneReconciler {
    pub fn new(megaphone: Arc<Megaphone>, ctx: Arc<ContextData>) -> Result<Self, Error> {
        Ok(Self {
            megaphone,
            ctx,
        })
    }

    fn pods(&self) -> Api<Pod> { Api::namespaced(self.ctx.client.clone(), self.cluster_namespace()) }
    fn services(&self) -> Api<Service> { Api::namespaced(self.ctx.client.clone(), self.cluster_namespace()) }
    fn workloads(&self) -> Api<Megaphone> { Api::namespaced(self.ctx.client.clone(), self.cluster_namespace()) }
    fn megactl(&self) -> MegactlService { MegactlService::new(self.pods()) }

    fn owner_ref(&self) -> OwnerReference {
        self.megaphone.controller_owner_ref(&()).unwrap()
    }

    fn cluster_namespace(&self) -> &str {
        self.megaphone
            .metadata
            .namespace
            .as_ref()
            .ok_or_else(|| Error::MissingObjectKey(".metadata.namespace"))
            .unwrap()
    }
    fn cluster_name(&self) -> &str {
        self.megaphone
            .metadata
            .name
            .as_ref()
            .ok_or_else(|| Error::MissingObjectKey(".metadata.names"))
            .unwrap()
    }


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

    fn create_pod(&self, idx: usize) -> MegaphonePod {
        let oref = self.owner_ref();
        let cluster_name = self.cluster_name();

        let virtual_agent_ids = (0..self.megaphone.spec.virtual_agents_per_node)
            .into_iter()
            .map(|virtual_agent_idx| Self::build_vagent_id(idx, virtual_agent_idx))
            .collect::<Vec<_>>();

        let env_vars = virtual_agent_ids.iter()
            .map(|virtual_agent_id| EnvVar {
                name: format!("megaphone_agent.virtual.{virtual_agent_id}"),
                value: Some(String::from("MASTER")),
                value_from: None,
            })
            .collect::<Vec<_>>();

        let pod_labels = virtual_agent_ids.iter()
            .flat_map(|virtual_agent_id| [
                (format!("megaphone-{virtual_agent_id}-write"), String::from("ON")),
                (format!("megaphone-{virtual_agent_id}-read"), String::from("ON")),
            ])
            .chain([
                (String::from("megaphone-cluster"), String::from(cluster_name)),
                (String::from("accepts-new-channels"), String::from("OFF")),
            ])
            .collect();

        let pod_name = format!("mgp-{cluster_name}-{idx}");

        MegaphonePod {
            name: pod_name.to_owned(),
            virtual_agent_ids,
            spec: Pod {
                metadata: ObjectMeta {
                    name: Some(pod_name.to_owned()),
                    namespace: Some(self.cluster_namespace().to_owned()),
                    owner_references: Some(vec![oref]),
                    labels: Some(pod_labels),
                    ..Default::default()
                },
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: pod_name.to_owned(),
                        image: Some(String::from(&self.megaphone.spec.image)),
                        resources: self.megaphone.spec.resources.clone()
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

    fn create_virtual_agent_service(&self, virtual_agent_id: &str, capability: &str) -> MegaphoneService {
        let cluster_name = self.cluster_name();
        let namespace = self.cluster_namespace();
        let oref = self.owner_ref();

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
            },
        }
    }

    fn create_cluster_service(&self) -> MegaphoneService {
        let cluster_name = self.cluster_name();
        let svc_name = format!("svc-{cluster_name}");
        MegaphoneService {
            name: svc_name.to_string(),
            spec: Service {
                metadata: ObjectMeta {
                    name: Some(svc_name.to_owned()),
                    namespace: Some(self.cluster_namespace().to_owned()),
                    owner_references: Some(vec![self.owner_ref()]),
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
                        (String::from("accepts-new-channels"), String::from("ON")),
                    ].into_iter().collect()),
                    ..Default::default()
                }),
                status: None,
            },
        }
    }

    fn compute_pod_labels(&self, vitual_agents: &[VirtualAgentItemDto]) -> BTreeMap<String, String> {
        let has_ready_masters = vitual_agents.iter()
            .find(|agent| matches!(agent.mode, VirtualAgentModeDto::Master) && !agent.warming_up)
            .is_some();

        vitual_agents.iter()
            .into_iter()
            .flat_map(|virtual_agent| [
                (format!("megaphone-{}-write", virtual_agent.name), if matches!(virtual_agent.mode, VirtualAgentModeDto::Master | VirtualAgentModeDto::Piped) { String::from("ON") } else { String::from("OFF") }),
                (format!("megaphone-{}-read", virtual_agent.name), if matches!(virtual_agent.mode, VirtualAgentModeDto::Master | VirtualAgentModeDto::Replica) { String::from("ON") } else { String::from("OFF") }),
            ])
            .chain([
                (String::from("megaphone-cluster"), String::from(self.cluster_name())),
                (String::from("accepts-new-channels"), if has_ready_masters { String::from("ON") } else { String::from("OFF") }),
            ])
            .collect()
    }

    fn is_send<T: Send>() -> bool { true }
    pub async fn reconcile(self) -> Result<Action, Error> {
        let current_workloads = self.megaphone
            .status
            .clone()
            .unwrap_or_default()
            .pods
            .len();

        Self::is_send::<Megaphone>();
        Self::is_send::<ContextData>();

        let pods_api = self.pods();
        let services_api = self.services();
        let workloads_api = self.workloads();
        let megactl_api = self.megactl();

        let mut new_pods = Vec::<String>::new();
        let mut new_services = Vec::<String>::new();
        for pod_idx in 0..self.megaphone.spec.replicas {
            let megaphone_pod = self.create_pod(pod_idx);
            let current_pod = pods_api.get_opt(&megaphone_pod.name).await
                .map_err(Error::PodCreationFailed)?;

            let megaphone_services = megaphone_pod.virtual_agent_ids.iter()
                .flat_map(|virtual_agent_id| [
                    self.create_virtual_agent_service(virtual_agent_id, "read"),
                    self.create_virtual_agent_service(virtual_agent_id, "write"),
                ])
                .collect::<Vec<_>>();

            match current_pod {
                None => {
                    let res = pods_api
                        .patch(
                            &megaphone_pod.name,
                            &PatchParams::apply("workload-Controller"),
                            &Patch::Apply(megaphone_pod.spec.clone()),
                        )
                        .await
                        .map_err(Error::PodCreationFailed);

                    match res {
                        Ok(_) => new_pods.push(megaphone_pod.name.clone()),
                        Err(e) => println!("Pod Creation Failed {:?}", e),
                    }
                }
                Some(current_pod_spec) if self.megaphone.spec.does_spec_change_require_pod_restart(&current_pod_spec) => {
                    todo!("Cluster upgrade is not yet implemented")
                }
                Some(current_pod_spec) => {
                    new_pods.push(megaphone_pod.name.clone());
                }
            }

            let agents = match megactl_api.list_agents(&megaphone_pod.name).await {
                Ok(agents) => agents,
                Err(err) => {
                    eprintln!("Could not find agents info - {err}");
                    continue;
                }
            };

            let update_labels = ObjectMeta {
                labels: Some(self.compute_pod_labels(&agents)),
                ..Default::default()
            }.into_request_partial::<Pod>();
            let patch_out = pods_api
                .patch_metadata(&megaphone_pod.name, &PatchParams::default(), &Patch::Merge(&update_labels))
                .await;

            match patch_out {
                Ok(pod) => println!("Pod meta correctly patched - {:?}", pod.metadata),
                Err(err) => eprintln!("Error applying label patch - {err}"),
            }

            for megaphone_svc in megaphone_services {
                let res = services_api.patch(
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
        let cluster_svc = self.create_cluster_service();
        let res = services_api.patch(
            &cluster_svc.name,
            &PatchParams::apply("workload-Controller"),
            &Patch::Apply(cluster_svc.spec.clone()),
        ).await;

        match res {
            Ok(_) => new_services.push(cluster_svc.name),
            Err(e) => println!("Service Creation Failed {:?}", e),
        }

        let update_status = json!({
            "status": MegaphoneStatus {
                pods: new_pods,
                services: new_services,
                cluster_status: MegaphoneClusterStatus::Idle,
                upgrade_spec: None,
            }
        });

        let res = workloads_api
            .patch_status(self.cluster_name(), &PatchParams::default(), &Patch::Merge(&update_status))
            .await;

        if let Err(err) = res {
            println!("Pod Creation Failed {:?}", err);
        }

        finalizer(&workloads_api, WORKLOAD_FINALIZER, self.megaphone.clone(), |event| async {
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
}