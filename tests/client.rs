use anyhow::Context;
use megaphone::dto::agent::BasicOutcomeDto;
use megaphone::dto::channel::{ChannelCreateReqDto, ChannelCreateResDto};
use megaphone::dto::message::EventDto;
use serde::de::DeserializeOwned;
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
            let status = resp.status();
            eprintln!("HTTP error {status} - {}", resp.text().await.unwrap_or_else(|err| err.to_string()));
            anyhow::bail!("Http failed with {status}")
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
            let status = resp.status();
            eprintln!("HTTP error {status} - {}", resp.text().await.unwrap_or_else(|err| err.to_string()));
            anyhow::bail!("Http failed with {status}")
        }
    }

    pub async fn read(&self, consumer_address: &str) -> anyhow::Result<EventDto> {
        let resp = reqwest::get(format!("http://{}:{}/read/{consumer_address}", self.host, self.port))
            .await
            .context("Error during channel write")?;

        if resp.status().is_success() {
            resp
                .json()
                .await
                .context("Error parsing response")
        } else {
            let status = resp.status();
            eprintln!("HTTP error {status} - {}", resp.text().await.unwrap_or_else(|err| err.to_string()));
            anyhow::bail!("Http failed with {status}")
        }
    }
}