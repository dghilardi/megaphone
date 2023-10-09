use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct ChannelCreateResDto {
    pub channel_id: String,
    pub agent_name: String,
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