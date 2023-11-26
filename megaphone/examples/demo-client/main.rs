use std::collections::HashMap;
use anyhow::anyhow;

use futures::StreamExt;
use serde::Deserialize;

use megaphone::client::{MegaphoneClient, StreamSpec};

#[derive(Deserialize)]
struct NewChatMessagePayload {
    message: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut client = MegaphoneClient::new("http://localhost:5173/read", 100);
    let mut msg_stream = client.new_unbounded_stream::<_, anyhow::Error, _, NewChatMessagePayload>(|channel| async {
        let http_client = reqwest::Client::new();

        let mut req_builder = http_client.post("http://localhost:5173/room/test");
        if let Some(channel) = channel {
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
    }).await?;

    while let Some(msg) = msg_stream.next().await {
        match msg {
            Ok(payload) => println!("message {}", payload.message),
            Err(err) => eprintln!("Error deserializing message: {err}"),
        }

    }
    println!("Stream ended");

    Ok(())
}