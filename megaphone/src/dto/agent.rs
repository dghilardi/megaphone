use std::net::SocketAddr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::service::agents_manager_service::VirtualAgentStatus;

#[derive(Serialize, Deserialize)]
pub struct VirtualAgentItemDto {
    pub name: String,
    pub since: DateTime<Utc>,
    pub warming_up: bool,
    pub mode: VirtualAgentModeDto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VirtualAgentModeDto {
    Master,
    Replica,
    Piped,
}

impl From<VirtualAgentStatus> for VirtualAgentModeDto {
    fn from(value: VirtualAgentStatus) -> Self {
        match value {
            VirtualAgentStatus::Master => Self::Master,
            VirtualAgentStatus::Replica { .. } => Self::Replica,
            VirtualAgentStatus::Piped => Self::Piped,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AddVirtualAgentReqDto {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct PipeVirtualAgentReqDto {
    pub name: String,
    pub target: SocketAddr,
}

#[derive(Serialize, Deserialize)]
pub struct BasicOutcomeDto {
    pub status: OutcomeStatus,
}

impl BasicOutcomeDto {
    pub fn ok() -> Self {
        Self {
            status: OutcomeStatus::Ok,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum OutcomeStatus {
    Ok,
}