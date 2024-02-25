use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct ChannelCreateResDto {
    pub channel_id: String,
    pub agent_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteBatchReqDto {
    pub channel_ids: HashSet<String>,
    pub messages: Vec<ChanMessage>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChanMessage {
    pub stream_id: String,
    pub body: serde_json::Value,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteBatchResDto {
    pub failures: Vec<MessageDeliveryFailure>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDeliveryFailure {
    pub channel: String,
    pub index: usize,
    pub reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChanExistsReqDto {
    pub channel_ids: HashSet<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChanExistsResDto {
    pub channel_ids: HashMap<String, bool>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsListParams {
    #[serde(default)]
    pub agents: HashSet<String>,
    #[serde(default)]
    pub skip: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInfoDto {
    pub channel_id: String,
    pub agent_id: String,
}

impl FromStr for ChannelInfoDto {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            channel_id: String::from(s),
            agent_id: s.split('.')
                .next()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow!("Cannot extract agent from {s}"))?,
        })
    }
}