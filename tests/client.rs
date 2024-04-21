use anyhow::Context;
use megaphone::dto::agent::BasicOutcomeDto;
use megaphone::dto::channel::{ChannelCreateReqDto, ChannelCreateResDto};
use serde::Serialize;

pub struct MegaphoneRestClient {
    host: String,
    port: u16,
}

impl MegaphoneRestClient {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: String::from(host),
            port
        }
    }

    pub async fn create(&self, req: &ChannelCreateReqDto) -> anyhow::Result<ChannelCreateResDto> {
        let client = reqwest::Client::new();
        client.post(format!("http://{}:{}/create", self.host, self.port))
            .json(req)
            .send()
            .await
            .context("Error during channel create")?
            .json::<ChannelCreateResDto>()
            .await
            .context("Error parsing response")
    }

    pub async fn write(&self, producer_address: &str, stream_id: &str, payload: impl Serialize) -> anyhow::Result<BasicOutcomeDto> {
        let client = reqwest::Client::new();
        client.post(format!("http://{}:{}/write/{producer_address}/{stream_id}", self.host, self.port))
            .json(&payload)
            .send()
            .await
            .context("Error during channel write")?
            .json()
            .await
            .context("Error parsing response")
    }
}