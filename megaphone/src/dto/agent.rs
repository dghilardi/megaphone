use serde::Serialize;

use crate::core::config::VirtualAgentMode;

#[derive(Serialize)]
pub struct VirtualAgentItemDto {
    pub name: String,
    pub mode: VirtualAgentModeDto,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VirtualAgentModeDto {
    Master,
    Replica,
}

impl From<VirtualAgentMode> for VirtualAgentModeDto {
    fn from(value: VirtualAgentMode) -> Self {
        match value {
            VirtualAgentMode::Master => Self::Master,
            VirtualAgentMode::Replica => Self::Replica,
        }
    }
}