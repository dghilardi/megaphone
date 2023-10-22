use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use crate::core::error::MegaphoneError;

#[derive(Serialize)]
pub struct ErrorDto {
    pub code: String,
}

impl From<MegaphoneError> for (StatusCode, Json<ErrorDto>) {
    fn from(err: MegaphoneError) -> Self {
        match err {
            MegaphoneError::NotFound => (StatusCode::NOT_FOUND, Json(ErrorDto { code: String::from("NOT_FOUND") })),
            MegaphoneError::Busy => (StatusCode::CONFLICT, Json(ErrorDto { code: String::from("BUSY") })),
            MegaphoneError::InternalError(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorDto { code: String::from("INTERNAL_SERVER_ERROR") })),
            MegaphoneError::BadRequest(_) => (StatusCode::BAD_REQUEST, Json(ErrorDto { code: String::from("BAD_REQUEST") })),
        }
    }
}