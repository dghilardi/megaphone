use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EventDto {
    pub stream_id: String,
    pub payload: serde_json::Value,
}

impl EventDto {
    pub fn new(
        stream_id: String,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            stream_id,
            payload,
        }
    }
}