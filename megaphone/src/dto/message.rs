use std::time::SystemTime;
use chrono::{DateTime, Utc};
use rand::{random, Rng};
use rand::distributions::Alphanumeric;
use serde::{Serialize, Deserialize};
use crate::service::megaphone_service::WithTimestamp;

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

impl WithTimestamp for EventDto {
    fn timestamp(&self) -> SystemTime {
        self.timestamp.into()
    }
}