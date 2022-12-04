use std::sync::Arc;
use std::time::Duration;

use axum::{BoxError, Router, routing::{get, post}};
use axum::body::StreamBody;
use axum::extract::{FromRef, Json, Path, State};
use axum::handler::Handler;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde_json::{json, Value};
use uuid::Uuid;
use crate::dto::message::EventDto;
use futures::StreamExt;
use tokio::sync::RwLock;
use crate::core::config::{compose_config, MegaphoneConfig};
use crate::dto::channel::ChannelCreateResDto;
use crate::dto::error::ErrorDto;
use crate::service::megaphone_service::MegaphoneService;

pub mod service;
mod dto;
mod core;

async fn create_handler(
    State(svc): State<MegaphoneService<EventDto>>,
    State(cfg): State<Arc<RwLock<MegaphoneConfig>>>,
) -> impl IntoResponse {
    let channel_id = svc.create_channel().await;
    let agent_name = cfg.read().await.agent_name.clone();
    Json(ChannelCreateResDto {
        channel_id,
        agent_name,
    })
}

async fn read_handler(
    Path(id): Path<String>,
    State(svc): State<MegaphoneService<EventDto>>,
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
    State(svc): State<MegaphoneService<EventDto>>,
    Json(body): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<ErrorDto>)> {
    let uuid = Uuid::parse_str(&channel_id).unwrap();
    svc.write_into_channel(uuid, EventDto::new(stream_id, body)).await?;
    Ok((StatusCode::CREATED, Json(json!({ "status": "ok" }))))
}

pub struct MegaphoneState<Evt> {
    megaphone_cfg: Arc<RwLock<MegaphoneConfig>>,
    megaphone_svc: MegaphoneService<Evt>,
}

impl <Evt> Clone for MegaphoneState<Evt> {
    fn clone(&self) -> Self {
        Self {
            megaphone_cfg: self.megaphone_cfg.clone(),
            megaphone_svc: self.megaphone_svc.clone(),
        }
    }
}

impl <Evt> FromRef<MegaphoneState<Evt>> for MegaphoneService<Evt> {
    fn from_ref(app_state: &MegaphoneState<Evt>) -> Self {
        app_state.megaphone_svc.clone()
    }
}

impl <Evt> FromRef<MegaphoneState<Evt>> for Arc<RwLock<MegaphoneConfig>> {
    fn from_ref(app_state: &MegaphoneState<Evt>) -> Self {
        app_state.megaphone_cfg.clone()
    }
}

fn spawn_buffer_cleaner(svc: MegaphoneService<EventDto>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            svc.drop_expired();
        }
    });
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let app_config: MegaphoneConfig = compose_config("megaphone", "megaphone")
        .expect("Error loading configuration");

    let service = MegaphoneState {
        megaphone_cfg: Arc::new(RwLock::new(app_config)),
        megaphone_svc: MegaphoneService::new(),
    };

    spawn_buffer_cleaner(service.megaphone_svc.clone());
    let app = Router::with_state(service)

        .route("/create", post(create_handler))
        .route("/write/:channel_id/:stream_id", post(write_handler))
        .route("/read/:id", get(read_handler));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .expect("Error starting server");
}