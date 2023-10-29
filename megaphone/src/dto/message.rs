use rand::{random, Rng};
use rand::distributions::Alphanumeric;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EventDto {
    #[serde(rename = "sid")]
    pub stream_id: String,
    #[serde(rename = "eid")]
    pub event_id: String,
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
            body,
        }
    }
}