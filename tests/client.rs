use anyhow::Context;
use megaphone::dto::channel::{ChannelCreateReqDto, ChannelCreateResDto};

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
}