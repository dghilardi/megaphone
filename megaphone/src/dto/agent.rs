use std::time::SystemTime;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::config::VirtualAgentMode;
use crate::service::agents_manager_service::VirtualAgentStatus;

#[derive(Serialize)]
pub struct VirtualAgentItemDto {
    pub name: String,
    pub since: DateTime<Utc>,
    pub mode: VirtualAgentModeDto,
}

#[derive(Serialize)]
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
            VirtualAgentStatus::Replica => Self::Replica,
            VirtualAgentStatus::Piped => Self::Piped,
        }
    }
}

#[derive(Deserialize)]
pub struct AddVirtualAgentReqDto {
    pub name: String,
    #[serde(flatten)]
    pub mode: VirtualAgentRegistrationMode,
}

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VirtualAgentRegistrationMode {
    Master,
    Replica { address: String },
}