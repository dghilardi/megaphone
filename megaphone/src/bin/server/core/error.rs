use axum::http::StatusCode;
use axum::Json;
use thiserror::Error;

use megaphone::dto::error::ErrorDto;

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

impl From<MegaphoneError> for (StatusCode, Json<ErrorDto>) {
    fn from(err: MegaphoneError) -> Self {
        match &err {
            MegaphoneError::NotFound => (StatusCode::NOT_FOUND, Json(ErrorDto { code: String::from(err.code()), message: String::from("Not found") })),
            MegaphoneError::Busy => (StatusCode::CONFLICT, Json(ErrorDto { code: String::from(err.code()), message: String::from("Busy") })),
            MegaphoneError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorDto { code: String::from(err.code()), message: format!("Internal error - {msg}") })),
            MegaphoneError::BadRequest(msg) => (StatusCode::BAD_REQUEST, Json(ErrorDto { code: String::from(err.code()), message: format!("Bad Request - {msg}") })),
            MegaphoneError::Timeout { .. } => (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorDto { code: String::from(err.code()), message: String::from("Timeout") })),
            MegaphoneError::Skipped => (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorDto { code: String::from(err.code()), message: String::from("Skipped") })),
        }
    }
}