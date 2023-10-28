use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;

use crate::dto::agent::{AddVirtualAgentReqDto, BasicOutcomeDto, PipeVirtualAgentReqDto, VirtualAgentItemDto, VirtualAgentModeDto};
use crate::dto::error::ErrorDto;
use crate::service::agents_manager_service::AgentsManagerService;

pub async fn list_virtual_agents(
    State(svc): State<AgentsManagerService>,
) -> impl IntoResponse {
    let agents = svc.list_agents()
        .map(|entry| VirtualAgentItemDto {
            name: entry.key().to_string(),
            since: entry.value().change_ts().into(),
            warming_up: entry.value().is_warming_up(),
            mode: VirtualAgentModeDto::from(entry.value().status()),
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
    State(svc): State<AgentsManagerService>,
    Json(req): Json<PipeVirtualAgentReqDto>,
) -> Result<(StatusCode, Json<BasicOutcomeDto>), (StatusCode, Json<ErrorDto>)> {
    Ok((StatusCode::ACCEPTED, Json(BasicOutcomeDto::ok())))
}