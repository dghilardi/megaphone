use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EventDto {
    #[serde(rename = "sid")]
    pub stream_id: String,
    pub body: serde_json::Value,
}

impl EventDto {
    pub fn new(
        stream_id: String,
        body: serde_json::Value,
    ) -> Self {
        Self {
            stream_id,
            body,
        }
    }
}