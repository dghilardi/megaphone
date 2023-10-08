use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Any error originating from the `kube-rs` crate
    #[error("Kubernetes reported error: {source}")]
    KubeError {
        #[from]
        source: kube::Error,
    },
    /// Error in user input or Echo resource definition, typically missing fields.
    #[error("Invalid Echo CRD: {0}")]
    UserInputError(String),
}