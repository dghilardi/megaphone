use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct ChannelCreateResDto {
    pub channel_id: String,
    pub agent_name: String,
}