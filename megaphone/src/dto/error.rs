use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use crate::core::error::MegaphoneError;

#[derive(Deserialize, Serialize)]
pub struct ErrorDto {
    pub code: String,
}

impl From<MegaphoneError> for (StatusCode, Json<ErrorDto>) {
    fn from(err: MegaphoneError) -> Self {
        match err {
            MegaphoneError::NotFound => (StatusCode::NOT_FOUND, Json(ErrorDto { code: String::from(err.code()) })),
            MegaphoneError::Busy => (StatusCode::CONFLICT, Json(ErrorDto { code: String::from(err.code()) })),
            MegaphoneError::InternalError(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorDto { code: String::from(err.code()) })),
            MegaphoneError::BadRequest(_) => (StatusCode::BAD_REQUEST, Json(ErrorDto { code: String::from(err.code()) })),
            MegaphoneError::Timeout { .. } => (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorDto { code: String::from(err.code()) })),
            MegaphoneError::Skipped => (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorDto { code: String::from(err.code()) })),
        }
    }
}