use kube::Client;

pub struct ContextData {
    /// Kubernetes client to make Kubernetes API requests with. Required for K8S resource management.
    pub client: Client,
}