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
    #[error("Timeout reached {secs}s")]
    Timeout { secs: usize },
    #[error("Skipped")]
    Skipped,
}

impl MegaphoneError {
    pub fn code(&self) -> &'static str {
        match self {
            MegaphoneError::NotFound => "NOT_FOUND",
            MegaphoneError::Busy => "BUSY",
            MegaphoneError::InternalError(_) => "INTERNAL_SERVER_ERROR",
            MegaphoneError::BadRequest(_) => "BAD_REQUEST",
            MegaphoneError::Timeout { .. } => "TIMEOUT",
            MegaphoneError::Skipped => "SKIPPED",
        }
    }
}