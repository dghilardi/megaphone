use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use futures::StreamExt;
use tokio::sync::mpsc;
use tonic::codegen::tokio_stream::wrappers;

use megaphone::dto::agent::{AddVirtualAgentReqDto, BasicOutcomeDto, PipeVirtualAgentReqDto, VirtualAgentItemDto, VirtualAgentModeDto};
use megaphone::dto::error::ErrorDto;
use megaphone::dto::message::EventDto;

use crate::core::error::MegaphoneError;
use crate::grpc::server::megaphone::sync_service_client::SyncServiceClient;
use crate::grpc::server::megaphone::SyncRequest;
use crate::service::agents_manager_service::{AgentsManagerService, SyncEvent};
use crate::service::megaphone_service::MegaphoneService;

pub async fn list_virtual_agents(
    State(svc): State<AgentsManagerService>,
    State(channels_mgr): State<MegaphoneService<EventDto>>,
) -> impl IntoResponse {
    let agents = svc.list_agents()
        .into_iter()
        .map(|(name, props)| VirtualAgentItemDto {
            since: props.change_ts().into(),
            warming_up: props.is_warming_up(),
            mode: VirtualAgentModeDto::from(props.status()),
            channels_count: channels_mgr.count_by_agent(&name),
            name,
        })
        .collect::<Vec<_>>();
    Json(agents)
}

pub async fn add_virtual_agent(
    State(svc): State<AgentsManagerService>,
    Json(req): Json<AddVirtualAgentReqDto>,
) -> Result<(StatusCode, Json<BasicOutcomeDto>), (StatusCode, Json<ErrorDto>)> {
    svc.add_master(&req.name)?;
    Ok((StatusCode::CREATED, Json(BasicOutcomeDto::ok())))
}

pub async fn pipe_virtual_agent(
    State(agent_mgr): State<AgentsManagerService>,
    State(channels_mgr): State<MegaphoneService<EventDto>>,
    Json(req): Json<PipeVirtualAgentReqDto>,
) -> Result<(StatusCode, Json<BasicOutcomeDto>), (StatusCode, Json<ErrorDto>)> {
    let mut client = SyncServiceClient::connect(req.target).await
        .map_err(|err| MegaphoneError::InternalError(format!("Error during connection establishment - {err}")))?;
    let (tx, rx) = mpsc::channel(500);
    tokio::spawn(async move {
        match client.forward_events(wrappers::ReceiverStream::new(rx).map(|evt| SyncRequest::from(evt))).await {
            Ok(ok) => log::info!("Pipe terminated with message - {}", ok.into_inner().message),
            Err(err) => log::error!("Pipe terminated with error - {err}"),
        }
    });
    agent_mgr.register_pipe(&req.name, tx.clone())?;
    for channel_id in channels_mgr.channel_ids_by_agent(&req.name) {
        let out = tx.send(SyncEvent::ChannelCreated { id: channel_id }).await;
        if let Err(err) = out {
            log::error!("Error registering channel - {err}")
        }
    }
    Ok((StatusCode::ACCEPTED, Json(BasicOutcomeDto::ok())))
}