use std::sync::Arc;
use std::time::Duration;

use axum::{BoxError, Json};
use axum::body::StreamBody;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures::StreamExt;
use tokio::sync::RwLock;

use megaphone::dto::agent::{BasicOutcomeDto, OutcomeStatus};
use megaphone::dto::channel::{ChanExistsReqDto, ChanExistsResDto, ChannelCreateReqDto, ChannelCreateResDto, ChannelInfoDto, ChannelsListParams, WriteBatchReqDto, WriteBatchResDto};
use megaphone::dto::error::ErrorDto;
use megaphone::dto::message::EventDto;

use crate::core::config::MegaphoneConfig;
use crate::core::error::MegaphoneError;
use crate::service::megaphone_service::MegaphoneService;

pub async fn create_handler(
    State(svc): State<MegaphoneService<EventDto>>,
    body_opt: Option<Json<ChannelCreateReqDto>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorDto>)> {
    let Json(req) = body_opt.unwrap_or_default();
    let (agent_name, channel_id, producer_address, protocols) = svc.create_channel(&req.protocols).await?;
    Ok(Json(ChannelCreateResDto {
        producer_address,
        consumer_address: String::from(&channel_id),
        channel_id,
        agent_name,
        protocols,
    }))
}

pub async fn read_handler(
    Path(channel_id): Path<String>,
    State(conf): State<Arc<RwLock<MegaphoneConfig>>>,
    State(svc): State<MegaphoneService<EventDto>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorDto>)> {
    let duration = {
        let conf_read = conf.read().await;
        conf_read.poll_duration_millis
    };
    let stream = svc
        .read_channel(channel_id, Duration::from_millis(duration))
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
) -> Result<(StatusCode, Json<BasicOutcomeDto>), (StatusCode, Json<ErrorDto>)> {
    svc.write_into_channel(&channel_id, EventDto::new(stream_id, body)).await?;
    Ok((StatusCode::CREATED, Json(BasicOutcomeDto {
        status: OutcomeStatus::Ok,
    })))
}

pub async fn write_batch_handler(
    State(svc): State<MegaphoneService<EventDto>>,
    Json(body): Json<WriteBatchReqDto>,
) -> Result<(StatusCode, Json<WriteBatchResDto>), (StatusCode, Json<ErrorDto>)> {
    let messages = body.messages.into_iter()
        .map(|message| EventDto::new(message.stream_id, message.body))
        .collect();

    let failures = svc.write_batch_into_channels(&body.channels.into_iter().collect::<Vec<_>>()[..], messages).await;
    Ok((StatusCode::CREATED, Json(WriteBatchResDto {
        failures,
    })))
}

pub async fn channel_exists_handler(
    State(svc): State<MegaphoneService<EventDto>>,
    Json(req): Json<ChanExistsReqDto>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorDto>)> {
    Ok(Json(ChanExistsResDto {
        channels: req.channels.into_iter()
            .map(|id| (id.clone(), svc.channel_exists(&id)))
            .collect(),
    }))
}

pub async fn channel_delete_handler(
    Path(channel_id): Path<String>,
    State(svc): State<MegaphoneService<EventDto>>,
) -> Result<Json<BasicOutcomeDto>, (StatusCode, Json<ErrorDto>)> {
    svc.drop_channel(&channel_id)?;
    Ok(Json(BasicOutcomeDto {
        status: OutcomeStatus::Ok,
    }))
}

pub async fn channels_list_handler(
    Query(params): Query<ChannelsListParams>,
    State(svc): State<MegaphoneService<EventDto>>,
) -> Result<Json<Vec<ChannelInfoDto>>, (StatusCode, Json<ErrorDto>)> {
    let channels = svc.list_channels(params.skip, params.limit)
        .map_err(|e| MegaphoneError::InternalError(format!("Error retrieving channels - {e}")))?;
    Ok(Json(channels))
}