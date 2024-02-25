use std::collections::HashMap;
use anyhow::anyhow;

use futures::StreamExt;
use serde::Deserialize;

use megaphone::client::{MegaphoneClient, model::StreamSpec};

#[derive(Deserialize)]
struct NewChatMessagePayload {
    message: String,
}

async fn initialize_chat_channel(channel_id: Option<String>) -> anyhow::Result<StreamSpec> {
    let http_client = reqwest::Client::new();

    let mut req_builder = http_client.post("http://localhost:3080/room/test");
    if let Some(channel) = channel_id {
        req_builder = req_builder.header("use-channel", channel);
    }
    let res = req_builder.send()
        .await?
        .json::<HashMap<String, String>>()
        .await?;

    Ok(StreamSpec {
        channel: res.get("channelUuid")
            .ok_or_else(|| anyhow!("channelId not found"))?
            .to_string(),
        streams: vec![String::from("new-message")],
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut client = MegaphoneClient::new("http://localhost:3080/read", 100);
    let mut msg_stream = client.new_unbounded_stream::<_, _, _, NewChatMessagePayload>(initialize_chat_channel).await?;

    while let Some(msg) = msg_stream.next().await {
        match msg {
            Ok(payload) => println!("message {}", payload.message),
            Err(err) => eprintln!("Error deserializing message: {err}"),
        }

    }
    println!("Stream ended");

    Ok(())
}