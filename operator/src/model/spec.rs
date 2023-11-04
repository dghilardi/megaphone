use k8s_openapi::api::core::v1::{Pod, ResourceRequirements};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Spec object for Workload
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
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
    pub fn does_spec_change_require_pod_restart(&self, pod: &Pod) -> bool {
        false
    }

    pub fn is_satisfied_by_pod(&self, pod: &Pod) -> bool {
        let Some(megaphone_container) = pod.spec.as_ref()
            .and_then(|spec| spec.containers.iter().find(|container| container.name.eq("megaphone"))) else {
            return false;
        };
        if megaphone_container.image.as_ref().map(|image| image.ne(&self.image)).unwrap_or(true) {
            return false;
        }
        true
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
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

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ResourceConstraints {
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

/// Status object for Workload
#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
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

#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MegaphoneClusterStatus {
    #[default]
    Idle,
    Upgrade,
}