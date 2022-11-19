pub mod service;

use std::sync::Arc;
use std::time::Duration;
use axum::{routing::{get, post}, Router};
use axum::body::StreamBody;
use axum::extract::{Path, State, Json};
use axum::handler::Handler;
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::AddExtension;
use axum::response::IntoResponse;
use serde_json::json;
use uuid::Uuid;
use crate::service::megaphone_service::MegaphoneService;

async fn create_handler(
    State(svc): State<Arc<MegaphoneService>>,
) -> String {
    svc.create_channel().await
}

async fn read_handler(
    Path(id): Path<String>,
    State(svc): State<Arc<MegaphoneService>>,
) -> impl IntoResponse {
    let uuid = Uuid::parse_str(&id).unwrap();
    let stream = svc.read_channel(uuid, Duration::from_secs(10)).await;
    let body = StreamBody::new(stream);

    body
}

async fn write_handler(
    Path(id): Path<String>,
    State(svc): State<Arc<MegaphoneService>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let uuid = Uuid::parse_str(&id).unwrap();
    svc.write_into_channel(uuid, serde_json::to_string(&body).unwrap()).await
        .expect("asd");
    (StatusCode::CREATED, Json(json!({ "status": "ok" })))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let service = Arc::new(MegaphoneService::new());

    let app = Router::with_state(service)

        .route("/create", post(create_handler))
        .route("/write/:id", post(write_handler))
        .route("/read/:id", get(read_handler));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .expect("Error starting server");
}