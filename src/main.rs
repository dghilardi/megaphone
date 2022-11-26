use std::sync::Arc;
use std::time::Duration;

use axum::{BoxError, Router, routing::{get, post}};
use axum::body::StreamBody;
use axum::extract::{Json, Path, State};
use axum::handler::Handler;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde_json::json;
use uuid::Uuid;
use crate::dto::message::EventDto;
use futures::StreamExt;
use crate::service::megaphone_service::MegaphoneService;

pub mod service;
mod dto;

async fn create_handler(
    State(svc): State<Arc<MegaphoneService<EventDto>>>,
) -> String {
    svc.create_channel().await
}

async fn read_handler(
    Path(id): Path<String>,
    State(svc): State<Arc<MegaphoneService<EventDto>>>,
) -> impl IntoResponse {
    let uuid = Uuid::parse_str(&id).unwrap();
    let stream = svc
        .read_channel(uuid, Duration::from_secs(10))
        .await
        .map(|evt| serde_json::to_string(&evt)
            .map(|mut s| {
                s.push('\n');
                s
            })
            .map_err(BoxError::from)
        );
    let body = StreamBody::new(stream);

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/x-ndjson".parse().unwrap());

    (headers, body)
}

async fn write_handler(
    Path((channel_id, stream_id)): Path<(String, String)>,
    State(svc): State<Arc<MegaphoneService<EventDto>>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let uuid = Uuid::parse_str(&channel_id).unwrap();
    svc.write_into_channel(uuid, EventDto::new(stream_id, body)).await
        .expect("asd");
    (StatusCode::CREATED, Json(json!({ "status": "ok" })))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let service = Arc::new(MegaphoneService::new());

    let app = Router::with_state(service)

        .route("/create", post(create_handler))
        .route("/write/:channel_id/:stream_id", post(write_handler))
        .route("/read/:id", get(read_handler));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .expect("Error starting server");
}