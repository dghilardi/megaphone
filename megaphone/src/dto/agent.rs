use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VirtualAgentItemDto {
    pub name: String,
    pub since: DateTime<Utc>,
    pub warming_up: bool,
    pub mode: VirtualAgentModeDto,
    pub channels_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VirtualAgentModeDto {
    Master,
    Replica,
    Piped,
}



#[derive(Serialize, Deserialize)]
pub struct AddVirtualAgentReqDto {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct PipeVirtualAgentReqDto {
    pub name: String,
    pub target: String,
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