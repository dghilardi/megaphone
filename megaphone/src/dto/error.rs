use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use crate::core::error::MegaphoneError;

#[derive(Deserialize, Serialize)]
pub struct ErrorDto {
    pub code: String,
    pub message: String,
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