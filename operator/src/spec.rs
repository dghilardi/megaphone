use kube_derive::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A custom resource
#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(group = "d71.dev", version = "v1", kind = "Megaphone", namespaced)]
pub struct MegaphoneSpec {
    pub image: String,
    pub replicas: i32,
}