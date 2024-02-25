use std::future::Future;
use std::sync::Arc;

use futures::Stream;
use futures::stream::StreamExt;
use hyper::{Client, Uri};
use hyper::body::HttpBody;
use hyper_tls::HttpsConnector;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::client::error::{DelayedResponseError, Error};
use crate::client::model::StreamSpec;
use crate::client::utils::circular_buffer::CircularBuffer;
use crate::dto::message::EventDto;

mod utils;
mod error;
pub mod model;

struct StreamSubscription {
    channel_id: String,
    stream_id: String,
    tx: UnboundedSender<EventDto>,
}

pub struct MegaphoneClient {
    url: Arc<RwLock<String>>,
    channel_id: Arc<RwLock<Option<String>>>,
    event_buffer: Arc<RwLock<CircularBuffer<String>>>,
    subscriptions: Arc<RwLock<Vec<StreamSubscription>>>,
}

impl MegaphoneClient {
    pub fn new(url: &str, buf_len: usize) -> Self {
        Self {
            url: Arc::new(RwLock::new(String::from(url))),
            channel_id: Arc::new(Default::default()),
            event_buffer: Arc::new(RwLock::new(CircularBuffer::new(buf_len))),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn new_unbounded_stream<Initializer, InitErr, Fut, Message>(&mut self, initializer: Initializer) -> Result<impl Stream<Item=Result<Message, serde_json::error::Error>>, InitErr>
        where
            Initializer: Fn(Option<String>) -> Fut,
            InitErr: From<Error>,
            Fut: Future<Output=Result<StreamSpec, InitErr>>,
            Message: DeserializeOwned,
    {
        let rx = self.initialize_stream(initializer).await?;
        let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
            .map(|msg| serde_json::from_value(msg.body));
        Ok(stream)
    }

    pub async fn delayed_response<Initializer, InitErr, Fut, Message>(&mut self, initializer: Initializer) -> Result<Message, DelayedResponseError>
        where
            Initializer: Fn(Option<String>) -> Fut,
            InitErr: From<Error> + ToString,
            Fut: Future<Output=Result<StreamSpec, InitErr>>,
            Message: DeserializeOwned,
    {
        let mut rx = self.initialize_stream(initializer).await
            .map_err(|err| DelayedResponseError::InitializationError(err.to_string()))?;

        let event = rx.recv().await
            .ok_or(DelayedResponseError::MissingResponse)?;

        serde_json::from_value(event.body)
            .map_err(|err| DelayedResponseError::DeserializationError(err.to_string()))
    }


    async fn initialize_stream<Initializer, InitErr, Fut>(&mut self, initializer: Initializer) -> Result<UnboundedReceiver<EventDto>, InitErr>
        where
            Initializer: Fn(Option<String>) -> Fut,
            InitErr: From<Error>,
            Fut: Future<Output=Result<StreamSpec, InitErr>>,
    {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<EventDto>();
        {
            let url_guard = self.url.read().await;
            let mut channel_guard = self.channel_id.write().await;
            let mut subscriptions_guard = self.subscriptions.write().await;
            let channel_spec = initializer(channel_guard.clone()).await?;
            for stream_id in channel_spec.streams {
                subscriptions_guard.push(StreamSubscription {
                    channel_id: channel_spec.channel.clone(),
                    stream_id,
                    tx: tx.clone(),
                });
            }
            if channel_guard.as_ref().map(|old_chan| old_chan.ne(&channel_spec.channel)).unwrap_or(true) {
                *channel_guard = Some(channel_spec.channel);
                drop(subscriptions_guard);
                drop(channel_guard);
                Self::spawn_reader(url_guard.as_str(), self.channel_id.clone(), self.event_buffer.clone(), self.subscriptions.clone()).await?;
            }
        }
        Ok(rx)
    }

    async fn spawn_reader(
        url: &str,
        channel_id: Arc<RwLock<Option<String>>>,
        event_buffer: Arc<RwLock<CircularBuffer<String>>>,
        subscriptions: Arc<RwLock<Vec<StreamSubscription>>>,
    ) -> Result<JoinHandle<()>, Error> {
        let channel_id_guard = channel_id.read().await;
        let current_channel_id = channel_id_guard.clone().expect("channel-id is not defined");
        drop(channel_id_guard);

        let read_uri: Uri = {
            let url = format!("{}/{}", url.trim_matches('/'), current_channel_id);
            url
                .parse()
                .map_err(|_err| Error::InvalidUrl { url })?
        };
        let handle = tokio::spawn(async move {
            loop {
                let connector = HttpsConnector::new();
                let client = Client::builder().build::<_, hyper::Body>(connector);

                let mut resp = match client.get(read_uri.clone()).await {
                    Ok(resp) => resp,
                    Err(err) => {
                        log::warn!("Error reading channel - {err}");
                        break;
                    }
                };
                while let Some(data_chunk_res) = resp.body_mut().data().await {
                    match data_chunk_res.map(|bytes| String::from_utf8(bytes.to_vec())) {
                        Err(err) => log::warn!("Error in received chunk - {err}"),
                        Ok(Err(err)) => log::warn!("Error parsing string from chunk - {err}"),
                        Ok(Ok(msg)) => {
                            for chunk_str in msg.split('\n').filter(|chunk| !chunk.is_empty()) {
                                match serde_json::from_str::<EventDto>(chunk_str) {
                                    Ok(evt) => {
                                        let sub_guard = subscriptions.read().await;
                                        let mut buf_guard = event_buffer.write().await;

                                        if !buf_guard.contains(&evt.event_id) {
                                            sub_guard
                                                .iter()
                                                .filter(|s| s.stream_id.eq(&evt.stream_id) && s.channel_id.eq(&current_channel_id))
                                                .filter(|s| !s.tx.is_closed())
                                                .for_each(|s| match s.tx.send(evt.clone()) {
                                                    Ok(_) => log::debug!("Message sent on stream {} listener", evt.stream_id),
                                                    Err(err) => log::error!("Error sending message to channel - {err}")
                                                });
                                            buf_guard.push(evt.event_id);
                                        }
                                    }
                                    Err(err) => log::warn!("Error deserializing chunk - {err}"),
                                }
                            }
                        }
                    }
                }
                if channel_id.read().await.as_ref().map(|channel| channel.ne(&current_channel_id)).unwrap_or(true) {
                    log::warn!("Channel id changed during polling. Ending poller for channel {current_channel_id}");
                    break;
                } else if subscriptions.read().await.iter().all(|s| !s.tx.is_closed() && s.channel_id.ne(&current_channel_id)) {
                    log::debug!("No subscriptions left for channel {current_channel_id}. Ending poller");
                    break;
                }
            }
            let mut channel_guard = channel_id.write().await;
            let mut subscriptions_guard = subscriptions.write().await;
            if channel_guard.as_ref().map(|c| c.eq(&current_channel_id)).unwrap_or(false) {
                *channel_guard = None;
            }
            subscriptions_guard.retain(|s| !s.tx.is_closed() && s.channel_id.ne(&current_channel_id));
        });
        Ok(handle)
    }
}