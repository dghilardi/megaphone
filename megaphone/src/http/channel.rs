use std::time::Duration;

use axum::{BoxError, Json};
use axum::body::StreamBody;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures::StreamExt;
use serde_json::{json, Value};

use crate::dto::channel::{ChanExistsReqDto, ChanExistsResDto, ChannelCreateResDto};
use crate::dto::error::ErrorDto;
use crate::dto::message::EventDto;
use crate::service::megaphone_service::MegaphoneService;

pub async fn create_handler(
    State(svc): State<MegaphoneService<EventDto>>,
) -> impl IntoResponse {
    let (agent_name, channel_id) = svc.create_channel().await;
    Json(ChannelCreateResDto {
        channel_id,
        agent_name,
    })
}

pub async fn read_handler(
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

pub async fn write_handler(
    Path((channel_id, stream_id)): Path<(String, String)>,
    State(svc): State<MegaphoneService<EventDto>>,
    Json(body): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<ErrorDto>)> {
    svc.write_into_channel(channel_id, EventDto::new(stream_id, body)).await?;
    Ok((StatusCode::CREATED, Json(json!({ "status": "ok" }))))
}

pub async fn channel_exists_handler(
    State(svc): State<MegaphoneService<EventDto>>,
    Json(req): Json<ChanExistsReqDto>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorDto>)> {
    Ok(Json(ChanExistsResDto {
        channel_ids: req.channel_ids.into_iter()
            .map(|id| (id.clone(), svc.channel_exists(&id)))
            .collect(),
    }))
}