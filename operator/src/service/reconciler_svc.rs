use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Display;
use std::sync::Arc;

use anyhow::Result;
use k8s_openapi::{api::core::v1::{Container, Pod, PodSpec}, apimachinery::pkg::apis::meta::v1::OwnerReference};
use k8s_openapi::api::core::v1::{EnvVar, Service, ServicePort, ServiceSpec};
use k8s_openapi::chrono::Utc;
use kube::{api::{Api, ObjectMeta, Patch, PatchParams, Resource}, ResourceExt, runtime::{
    controller::Action,
    finalizer::{Event as Finalizer, finalizer},
}};
use kube::api::{DeleteParams, ListParams};
use kube::core::PartialObjectMetaExt;
use rand::prelude::IteratorRandom;
use rand::random;
use regex::Regex;
use serde_json::json;
use tokio::time::Duration;

use megaphone::dto::agent::{VirtualAgentItemDto, VirtualAgentModeDto};

use crate::model::context::ContextData;
use crate::model::error::Error;
use crate::model::spec::{Megaphone, MegaphoneClusterStatus, MegaphoneStatus};
use crate::service::megactl_svc::MegactlService;

pub static WORKLOAD_FINALIZER: &str = "megaphone.d71.dev";

pub static LABEL_CLUSTER_NAME: &str = "megaphone-cluster";
pub static LABEL_NODE_NAME: &str = "megaphone-node";
pub static LABEL_ACCEPTS_NEW_CHANNELS: &str = "accepts-new-channels";

pub static LABEL_VALUE_ON: &str = "ON";
pub static LABEL_VALUE_OFF: &str = "OFF";

static CONNECTION_LABELS_REGEX: &str = r#"^(accepts-new-channels|megaphone-[a-zA-Z0-9]+-read|megaphone-[a-zA-Z0-9]+-write)$"#;

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

        let pod_name = format!("mgp-{cluster_name}-{idx}");

        let pod_labels = virtual_agent_ids.iter()
            .flat_map(|virtual_agent_id| [
                (format!("megaphone-{virtual_agent_id}-write"), String::from(LABEL_VALUE_ON)),
                (format!("megaphone-{virtual_agent_id}-read"), String::from(LABEL_VALUE_ON)),
            ])
            .chain([
                (String::from(LABEL_CLUSTER_NAME), String::from(cluster_name)),
                (String::from(LABEL_NODE_NAME), String::from(&pod_name)),
                (String::from(LABEL_ACCEPTS_NEW_CHANNELS), String::from(LABEL_VALUE_OFF)),
            ])
            .collect();


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
                        name: String::from("megaphone"),
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
                        (String::from(LABEL_CLUSTER_NAME), cluster_name.to_owned()),
                        (format!("megaphone-{virtual_agent_id}-{capability}"), String::from(LABEL_VALUE_ON)),
                    ].into_iter().collect()),
                    ..Default::default()
                }),
                status: None,
            },
        }
    }

    fn create_internal_pod_service(&self, pod_name: &str) -> MegaphoneService {
        let cluster_name = self.cluster_name();
        let namespace = self.cluster_namespace();
        let oref = self.owner_ref();

        let svc_name = format!("{pod_name}");
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
                        port: 3001,
                        ..Default::default()
                    }]),
                    selector: Some([
                        (String::from(LABEL_CLUSTER_NAME), cluster_name.to_owned()),
                        (String::from(LABEL_NODE_NAME), String::from(pod_name)),
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
                        (String::from(LABEL_CLUSTER_NAME), cluster_name.to_owned()),
                        (String::from(LABEL_ACCEPTS_NEW_CHANNELS), String::from(LABEL_VALUE_ON)),
                    ].into_iter().collect()),
                    ..Default::default()
                }),
                status: None,
            },
        }
    }

    fn compute_pod_labels(&self, vitual_agents: &[VirtualAgentItemDto], terminating: bool) -> BTreeMap<String, String> {
        let has_ready_masters = vitual_agents.iter()
            .find(|agent| matches!(agent.mode, VirtualAgentModeDto::Master) && !agent.warming_up)
            .is_some();

        vitual_agents.iter()
            .into_iter()
            .flat_map(|virtual_agent| [
                (format!("megaphone-{}-write", virtual_agent.name), match virtual_agent.mode {
                    VirtualAgentModeDto::Master => String::from(LABEL_VALUE_ON),
                    VirtualAgentModeDto::Replica if virtual_agent.since + Duration::from_secs(50) < Utc::now() => String::from(LABEL_VALUE_ON),
                    VirtualAgentModeDto::Replica => String::from(LABEL_VALUE_OFF),
                    VirtualAgentModeDto::Piped if virtual_agent.since + Duration::from_secs(60) < Utc::now() => String::from(LABEL_VALUE_OFF),
                    VirtualAgentModeDto::Piped => String::from(LABEL_VALUE_ON),
                }),
                (format!("megaphone-{}-read", virtual_agent.name), match virtual_agent.mode {
                    VirtualAgentModeDto::Master => String::from(LABEL_VALUE_ON),
                    VirtualAgentModeDto::Replica if virtual_agent.since + Duration::from_secs(30) < Utc::now() => String::from(LABEL_VALUE_ON),
                    VirtualAgentModeDto::Replica => String::from(LABEL_VALUE_OFF),
                    VirtualAgentModeDto::Piped if virtual_agent.since + Duration::from_secs(40) < Utc::now() => String::from(LABEL_VALUE_OFF),
                    VirtualAgentModeDto::Piped => String::from(LABEL_VALUE_ON),
                }),
            ])
            .chain([
                (String::from(LABEL_CLUSTER_NAME), String::from(self.cluster_name())),
                (String::from(LABEL_ACCEPTS_NEW_CHANNELS), if has_ready_masters && !terminating { String::from(LABEL_VALUE_ON) } else { String::from(LABEL_VALUE_OFF) }),
            ])
            .collect()
    }

    async fn tear_down_pod<I>(&self, pod: &Pod, other_nodes_urls: I) -> Result<(), Error>
        where I: IntoIterator,
              I::Item: ToString,
    {
        let pod_name = pod.metadata.name.as_ref()
            .ok_or_else(|| Error::InternalError(String::from("Cannot read pod name")))?;

        if pod.metadata.labels.as_ref().and_then(|labels| labels.get(LABEL_ACCEPTS_NEW_CHANNELS)).map(|value| value.eq(LABEL_VALUE_ON)).unwrap_or(false) {
            let update_labels = ObjectMeta {
                labels: Some(
                    [(String::from(LABEL_ACCEPTS_NEW_CHANNELS), String::from(LABEL_VALUE_OFF))]
                        .into_iter()
                        .collect()
                ),
                ..Default::default()
            }.into_request_partial::<Pod>();

            self.pods()
                .patch_metadata(pod_name, &PatchParams::default(), &Patch::Merge(&update_labels))
                .await
                .map_err(|err| Error::PodDeletionFailed(err))?;
        }

        let pipe_urls = other_nodes_urls.into_iter().map(|s| s.to_string()).collect::<HashSet<String>>();
        if !pipe_urls.is_empty() {
            let megactl = self.megactl();
            let pod_agents = megactl.list_agents(pod_name).await
                .map_err(|err| Error::InternalError(format!("Cannot list agents for pod {pod_name} - {err}")))?;

            for agent in pod_agents {
                match agent.mode {
                    VirtualAgentModeDto::Master |
                    VirtualAgentModeDto::Replica => {
                        let pipe_url = pipe_urls.iter().choose(&mut rand::thread_rng()).expect("Error selecting url");
                        let out = megactl.pipe_agent(pod_name, &agent.name, pipe_url).await;
                        match out {
                            Ok(_) => println!("Agent {} piped to {pipe_url}", agent.name),
                            Err(err) => eprintln!("Error piping agent - {err}"),
                        }
                    }
                    VirtualAgentModeDto::Piped => {}
                }
            }
        }

        Ok(())
    }

    async fn align_labels(&self, pod: &Pod, status: MegaphonePodStatus) -> Result<Vec<VirtualAgentItemDto>, Error> {
        let pod_name = pod.metadata.name.as_ref()
            .ok_or_else(|| Error::InternalError(String::from("Cannot read pod name")))?;

        let megactl = self.megactl();
        let pod_agents = megactl.list_agents(pod_name).await
            .map_err(|err| Error::InternalError(format!("Cannot list agents for pod {pod_name} - {err}")))?;

        let labels = self.compute_pod_labels(&pod_agents, matches!(status, MegaphonePodStatus::TearingDown));
        let update_labels = ObjectMeta {
            labels: Some(labels),
            ..Default::default()
        }.into_request_partial::<Pod>();

        self.pods()
            .patch_metadata(pod_name, &PatchParams::default(), &Patch::Merge(&update_labels))
            .await
            .map_err(|err| Error::PodDeletionFailed(err))?;
        Ok(pod_agents)
    }

    pub async fn reconcile(self) -> Result<Action, Error> {
        let current_workloads = self.megaphone
            .status
            .clone()
            .unwrap_or_default()
            .pods
            .len();

        let pods_api = self.pods();
        let services_api = self.services();
        let workloads_api = self.workloads();
        let megactl_api = self.megactl();

        let mut new_pods = Vec::<String>::new();
        let mut new_services = Vec::<String>::new();

        let mut pods_status = self.determine_cluster_status().await?;

        let mut deleted_pods = HashSet::new();
        let connection_labels_regex = Regex::new(CONNECTION_LABELS_REGEX).unwrap();
        for pod in pods_status.get(&MegaphonePodStatus::TearingDown).map(|v| &v[..]).unwrap_or_default() {
            let Some(labels) = &pod.metadata.labels else {
                continue;
            };
            let completely_disconnected = labels.iter()
                .filter(|(k, _)| connection_labels_regex.is_match(k))
                .all(|(_k, v)| v.eq(LABEL_VALUE_OFF));

            if completely_disconnected {
                let Some(pod_name) = &pod.metadata.name else {
                    continue;
                };
                println!("All connection labels are off for pod {pod_name}");
                pods_api.delete(pod_name, &DeleteParams::default()).await
                    .map_err(Error::PodDeletionFailed)?;

                deleted_pods.insert(pod_name.to_string());
            }
        }

        if let Some(tearing_down_pods) = pods_status.get_mut(&MegaphonePodStatus::TearingDown) {
            tearing_down_pods.retain(|pod| pod.metadata.name.as_ref().map(|name| deleted_pods.contains(name)).unwrap_or(false));
        }

        let total_pods_count = pods_status.values().map(|v| v.len()).sum::<usize>();
        let max_surge = 1;

        if total_pods_count < self.megaphone.spec.replicas {
            for i in 0..self.megaphone.spec.replicas - total_pods_count {
                let megaphone_pod = self.create_pod(random());

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
        } else if total_pods_count > self.megaphone.spec.replicas {
            let to_delete = [
                pods_status.get(&MegaphonePodStatus::WarmingUp).map(|v| &v[..]).unwrap_or_default(),
                pods_status.get(&MegaphonePodStatus::Active).map(|v| &v[..]).unwrap_or_default(),
            ]
                .into_iter()
                .flatten()
                .take(total_pods_count- self.megaphone.spec.replicas);

            for pod in to_delete {
                self.tear_down_pod(pod, Vec::<String>::new()).await?;
            }
        }

        let tear_down_list = pods_status.get(&MegaphonePodStatus::TearingDown)
            .map(|v| &v[..])
            .or_else(|| pods_status.get(&MegaphonePodStatus::QueuedForTearDown).map(|v| &v[0..max_surge]));


        let pipe_targets = pods_status.get(&MegaphonePodStatus::Active)
            .or_else(|| pods_status.get(&MegaphonePodStatus::QueuedForTearDown))
            .map(|v| v.iter().flat_map(|pod| pod.metadata.name.as_ref()).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|pod_name| format!("http://{pod_name}.{}.svc.cluster.local:3001", self.cluster_namespace()))
            .collect::<Vec<_>>();

        for pod in tear_down_list.unwrap_or_default() {
            self.tear_down_pod(pod, pipe_targets.iter()).await?;
        }

        let mut all_virtual_agents_ids = HashSet::new();
        let mut all_pod_names = HashSet::new();
        for (status, pods) in &pods_status {
            for pod in pods {
                let virtual_agents = self.align_labels(pod, *status).await?;
                for agent in virtual_agents {
                    all_virtual_agents_ids.insert(agent.name);
                }
                if let Some(pod_name) = &pod.metadata.name {
                    all_pod_names.insert(pod_name.to_string());
                }
            }
        }


        let megaphone_agent_services = all_virtual_agents_ids.iter()
            .flat_map(|virtual_agent_id| [
                self.create_virtual_agent_service(virtual_agent_id, "read"),
                self.create_virtual_agent_service(virtual_agent_id, "write"),
            ])
            .chain(
                all_pod_names.into_iter()
                    .map(|pod_name| self.create_internal_pod_service(&pod_name))
            )
            .collect::<Vec<_>>();

        for megaphone_svc in megaphone_agent_services {
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
        let status = MegaphoneStatus {
            pods: new_pods,
            services: new_services,
            cluster_status: MegaphoneClusterStatus::Idle,
            upgrade_spec: None,
        };

        let update_status = json!({
            "status": status
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

        let is_rollup_completed = pods_status.iter()
            .filter(|(status, _)| !matches!(status, MegaphonePodStatus::Active))
            .all(|(_, pods)| pods.is_empty());

        if is_rollup_completed {
            Ok(Action::requeue(Duration::from_secs(300)))
        } else {
            Ok(Action::requeue(Duration::from_secs(10)))
        }
    }

    async fn determine_cluster_status(&self) -> Result<HashMap<MegaphonePodStatus, Vec<Pod>>, Error> {
        let pods_api = self.pods();
        let params = ListParams::default().labels(&format!("{LABEL_CLUSTER_NAME}={}", self.cluster_name()));
        let pods = pods_api.list(&params).await
            .map_err(|err| Error::MissingObjectKey("Cannot find cluster pods"))?
            .into_iter()
            .map(|pod| (self.determine_pod_status(&pod), pod))
            .fold(HashMap::new(), |mut acc, (state, pod)| {
                acc
                    .entry(state)
                    .or_insert_with(Vec::new)
                    .push(pod);
                acc
            });

        Ok(pods)
    }

    fn determine_pod_status(&self, pod: &Pod) -> MegaphonePodStatus {
        let accepts_new_channels = pod.labels()
            .get(LABEL_ACCEPTS_NEW_CHANNELS)
            .map(|value| value.eq(LABEL_VALUE_ON))
            .unwrap_or(false);

        let satisfies_spec = self.megaphone.spec.is_satisfied_by_pod(pod);

        match (accepts_new_channels, satisfies_spec) {
            (true, true) => MegaphonePodStatus::Active,
            (true, false) => MegaphonePodStatus::QueuedForTearDown,
            (false, true) => MegaphonePodStatus::WarmingUp,
            (false, false) => MegaphonePodStatus::TearingDown,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
enum MegaphonePodStatus {
    Active,
    WarmingUp,
    TearingDown,
    QueuedForTearDown,
}