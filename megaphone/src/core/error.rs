use thiserror::Error;

#[derive(Debug, Error)]
pub enum MegaphoneError {
    #[error("Not Found")]
    NotFound,
    #[error("Resource is busy")]
    Busy,
    #[error("Internal Error - {0}")]
    InternalError(String),
    #[error("Bad Request - {0}")]
    BadRequest(String),
}