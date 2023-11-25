use chrono::{DateTime, Utc};
use rand::Rng;
use rand::distributions::Alphanumeric;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventDto {
    #[serde(rename = "sid")]
    pub stream_id: String,
    #[serde(rename = "eid")]
    pub event_id: String,
    #[serde(rename = "ts")]
    pub timestamp: DateTime<Utc>,
    pub body: serde_json::Value,
}

impl EventDto {
    pub fn new(
        stream_id: String,
        body: serde_json::Value,
    ) -> Self {
        Self {
            stream_id,
            event_id: rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(23)
                .map(char::from)
                .collect(),
            timestamp: Utc::now(),
            body,
        }
    }
}