use k8s_openapi::api::core::v1::ResourceRequirements;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Spec object for Workload
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "d71.dev", version = "v1", kind = "Megaphone", namespaced)]
#[kube(status = "MegaphoneStatus")]
#[kube(scale = r#"{"specReplicasPath":".spec.replicas", "statusReplicasPath":".status.replicas"}"#)]
pub struct MegaphoneSpec {
    pub image: String,
    pub replicas: usize,
    pub resources: Option<ResourcesSpec>,
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
pub struct MegaphoneStatus {
    pub pods: Vec<String>,
}