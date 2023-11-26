use k8s_openapi::api::core::v1::{Pod, ResourceRequirements};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Spec object for Workload
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[kube(group = "d71.dev", version = "v1", kind = "Megaphone", namespaced)]
#[kube(status = "MegaphoneStatus")]
#[kube(scale = r#"{"specReplicasPath":".spec.replicas", "statusReplicasPath":".status.replicas"}"#)]
#[serde(rename_all = "camelCase")]
pub struct MegaphoneSpec {
    pub image: String,
    pub replicas: usize,
    #[schemars(range(min = 1))]
    pub virtual_agents_per_node: usize,
    pub resources: Option<ResourcesSpec>,
}

impl MegaphoneSpec {
    pub fn is_satisfied_by_pod(&self, pod: &Pod) -> bool {
        let Some(megaphone_container) = pod.spec.as_ref()
            .and_then(|spec| spec.containers.iter().find(|container| container.name.eq("megaphone"))) else {
            return false;
        };
        if megaphone_container.image.as_ref().map(|image| image.ne(&self.image)).unwrap_or(true) {
            return false;
        }
        if let Some(ResourceRequirements { limits: Some(current_limits), .. }) = &megaphone_container.resources {
            if let Some(ResourcesSpec { limits: Some(new_limits), .. }) = &self.resources {
                match (current_limits.get("cpu"), new_limits.cpu.as_ref().map(|s| format!("\"{s}\"")).and_then(|cpu| serde_json::from_str::<Quantity>(&cpu).ok())) {
                    (Some(current), Some(required)) if required.ne(current) => {
                        log::debug!("cpu required limits are not satisfied, required: {required:?} current: {current:?}");
                        return false;
                    }
                    (Some(_current), Some(_required)) => log::debug!("cpu required limits are already satisfied"),
                    (None, Some(_required)) => log::warn!("Cannot read current pod cpu limits"),
                    (curr, req) => log::debug!("Unhandled {curr:?} {req:?}"),
                }
                match (current_limits.get("memory"), new_limits.memory.as_ref().map(|s| format!("\"{s}\"")).and_then(|memory| serde_json::from_str::<Quantity>(&memory).ok())) {
                    (Some(current), Some(required)) if required.ne(current) => {
                        log::debug!("memory required limits are not satisfied, required: {required:?} current: {current:?}");
                        return false;
                    }
                    (Some(_current), Some(_required)) => log::debug!("memory required limits are already satisfied"),
                    (None, Some(_required)) => log::warn!("Cannot read current pod memory limits"),
                    (curr, req) => log::debug!("Unhandled {curr:?} {req:?}"),
                }
            }
        } else {
            log::warn!("Cannot read pod resources")
        }
        if let Some(ResourceRequirements { requests: Some(current_requests), .. }) = &megaphone_container.resources {
            if let Some(ResourcesSpec { requests: Some(requests), .. }) = &self.resources {
                match (current_requests.get("cpu"), requests.cpu.as_ref().map(|s| format!("\"{s}\"")).and_then(|cpu| serde_json::from_str::<Quantity>(&cpu).ok())) {
                    (Some(current), Some(required)) if required.ne(current) => {
                        log::debug!("cpu required requests are not satisfied, required: {required:?} current: {current:?}");
                        return false;
                    }
                    (Some(_current), Some(_required)) => log::debug!("cpu required requests are already satisfied"),
                    (None, Some(_required)) => log::warn!("Cannot read current pod cpu requests"),
                    (curr, req) => log::debug!("Unhandled {curr:?} {req:?}"),
                }
                match (current_requests.get("memory"), requests.memory.as_ref().map(|s| format!("\"{s}\"")).and_then(|memory| serde_json::from_str::<Quantity>(&memory).ok())) {
                    (Some(current), Some(required)) if required.ne(current) => {
                        log::debug!("memory required requests are not satisfied, required: {required:?} current: {current:?}");
                        return false;
                    }
                    (Some(_current), Some(_required)) => log::debug!("memory required requests are already satisfied"),
                    (None, Some(_required)) => log::warn!("Cannot read current pod memory requests"),
                    (curr, req) => log::debug!("Unhandled {curr:?} {req:?}"),
                }
            }
        } else {
            log::warn!("Cannot read pod resources")
        }
        true
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ResourcesSpec {
    limits: Option<ResourceConstraints>,
    requests: Option<ResourceConstraints>,
}

impl From<ResourcesSpec> for ResourceRequirements {
    fn from(value: ResourcesSpec) -> Self {
        Self {
            limits: value.limits.map(|limits| [
                (String::from("cpu"), limits.cpu.map(Quantity)),
                (String::from("memory"), limits.memory.map(Quantity)),
            ]
                .into_iter()
                .flat_map(|(k, maybe_v)| maybe_v.map(|v| (k, v)))
                .collect()
            ),
            requests: value.requests.map(|limits| [
                (String::from("cpu"), limits.cpu.map(Quantity)),
                (String::from("memory"), limits.memory.map(Quantity)),
            ]
                .into_iter()
                .flat_map(|(k, maybe_v)| maybe_v.map(|v| (k, v)))
                .collect()
            ),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ResourceConstraints {
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

/// Status object for Workload
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MegaphoneStatus {
    pub pods: Vec<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub cluster_status: MegaphoneClusterStatus,
    #[serde(default)]
    pub upgrade_spec: Option<MegaphoneSpec>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MegaphoneClusterStatus {
    #[default]
    Idle,
    Upgrade,
}