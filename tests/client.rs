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
        let resp = client.post(format!("http://{}:{}/create", self.host, self.port))
            .json(req)
            .send()
            .await
            .context("Error during channel create")?;

        if resp.status().is_success() {
            resp
                .json::<ChannelCreateResDto>()
                .await
                .context("Error parsing response")
        } else {
            anyhow::bail!("Http failed with {}", resp.status())
        }
    }

    pub async fn write(&self, producer_address: &str, stream_id: &str, payload: impl Serialize) -> anyhow::Result<BasicOutcomeDto> {
        let client = reqwest::Client::new();
        let resp = client.post(format!("http://{}:{}/write/{producer_address}/{stream_id}", self.host, self.port))
            .json(&payload)
            .send()
            .await
            .context("Error during channel write")?;
        
        if resp.status().is_success() {
            resp
                .json()
                .await
                .context("Error parsing response")
        } else {
            anyhow::bail!("Http failed with {}", resp.status())
        }
    }
}