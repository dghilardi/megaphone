use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to create Pod: {0}")]
    PodCreationFailed(#[source] kube::Error),
    #[error("Failed to delete Pod: {0}")]
    PodDeletionFailed(#[source] kube::Error),
    #[error("MissingObjectKey: {0}")]
    MissingObjectKey(&'static str),
    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),
    #[error("Internal error - {0}")]
    InternalError(String),
}