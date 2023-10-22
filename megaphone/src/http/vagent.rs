use axum::extract::State;
use axum::Json;
use axum::response::IntoResponse;

use crate::dto::agent::{VirtualAgentItemDto, VirtualAgentModeDto};
use crate::service::agents_manager_service::AgentsManagerService;

pub async fn list_virtual_agents(
    State(svc): State<AgentsManagerService>,
) -> impl IntoResponse {
    let agents = svc.list_agents()
        .map(|entry| VirtualAgentItemDto {
            name: entry.key().to_string(),
            mode: VirtualAgentModeDto::from(entry.value().clone()),
        })
        .collect::<Vec<_>>();
    Json(agents)
}