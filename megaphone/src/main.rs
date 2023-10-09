use std::sync::Arc;
use std::time::Duration;

use axum::{BoxError, Router, routing::{get, post}};
use axum::body::StreamBody;
use axum::extract::{FromRef, Json, Path, State};
use axum::handler::Handler;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures::StreamExt;
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::core::config::{compose_config, MegaphoneConfig};
use crate::dto::channel::{ChanExistsReqDto, ChanExistsResDto, ChannelCreateResDto};
use crate::dto::error::ErrorDto;
use crate::dto::message::EventDto;
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
    Path(channel_id): Path<String>,
    State(svc): State<MegaphoneService<EventDto>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorDto>)> {
    let stream = svc
        .read_channel(channel_id, Duration::from_secs(10))
        .await?
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

    Ok((headers, body))
}

async fn write_handler(
    Path((channel_id, stream_id)): Path<(String, String)>,
    State(svc): State<MegaphoneService<EventDto>>,
    Json(body): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<ErrorDto>)> {
    svc.write_into_channel(channel_id, EventDto::new(stream_id, body)).await?;
    Ok((StatusCode::CREATED, Json(json!({ "status": "ok" }))))
}

async fn channel_exists_handler(
    State(svc): State<MegaphoneService<EventDto>>,
    Json(req): Json<ChanExistsReqDto>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorDto>)> {
    Ok(Json(ChanExistsResDto {
        channel_ids: req.channel_ids.into_iter()
            .map(|id| (id.clone(), svc.channel_exists(&id)))
            .collect(),
    }))
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
    env_logger::init();
    let app_config: MegaphoneConfig = compose_config("megaphone", "megaphone")
        .expect("Error loading configuration");

    let service = MegaphoneState {
        megaphone_cfg: Arc::new(RwLock::new(app_config)),
        megaphone_svc: MegaphoneService::new(),
    };

    spawn_buffer_cleaner(service.megaphone_svc.clone());
    let app = Router::new()

        .route("/create", post(create_handler))
        .route("/write/:channel_id/:stream_id", post(write_handler))
        .route("/read/:id", get(read_handler))
        .route("/channelsExists", post(channel_exists_handler))
        .with_state(service);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .expect("Error starting server");
}