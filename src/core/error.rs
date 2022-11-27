use thiserror::Error;

#[derive(Debug, Error)]
pub enum MegaphoneError {
    #[error("Not Found")]
    NotFound,
    #[error("Internal Error")]
    InternalError,
}