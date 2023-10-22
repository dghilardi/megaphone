use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use serde_json::{json, Value};

use crate::dto::agent::{AddVirtualAgentReqDto, VirtualAgentItemDto, VirtualAgentModeDto, VirtualAgentRegistrationMode};
use crate::dto::error::ErrorDto;
use crate::service::agents_manager_service::AgentsManagerService;

pub async fn list_virtual_agents(
    State(svc): State<AgentsManagerService>,
) -> impl IntoResponse {
    let agents = svc.list_agents()
        .map(|entry| VirtualAgentItemDto {
            name: entry.key().to_string(),
            since: entry.value().change_ts().into(),
            mode: VirtualAgentModeDto::from(entry.value().status()),
        })
        .collect::<Vec<_>>();
    Json(agents)
}

pub async fn add_virtual_agent(
    State(svc): State<AgentsManagerService>,
    Json(req): Json<AddVirtualAgentReqDto>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<ErrorDto>)> {
    match req.mode {
        VirtualAgentRegistrationMode::Master => svc.add_master(&req.name)?,
        VirtualAgentRegistrationMode::Replica { .. } => todo!("Not yet implemented")
    }
    Ok((StatusCode::CREATED, Json(json!({ "status": "ok" }))))
}