use std::collections::HashMap;
use std::ops::Add;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use dashmap::DashMap;
use futures::FutureExt;
use metrics::{counter, histogram};
use rand::distributions::Alphanumeric;
use rand::Rng;
use tokio::sync::mpsc::error::{SendTimeoutError, TrySendError};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::core::config::{WebHook, WebHookType};
use megaphone::dto::channel::MessageDeliveryFailure;
use megaphone::dto::message::EventDto;
use megaphone::model::constants::protocols;
use megaphone::model::feature::Feature;
use serde_json::json;

use crate::core::error::MegaphoneError;
use crate::service::agents_manager_service::{AgentsManagerService, SyncEvent};

pub const CHANNEL_CREATED_METRIC_NAME: &str = "megaphone_channel_created";
pub const CHANNEL_DISPOSED_METRIC_NAME: &str = "megaphone_channel_disposed";
pub const CHANNEL_DURATION_METRIC_NAME: &str = "megaphone_channel_duration";
pub const MESSAGES_RECEIVED_METRIC_NAME: &str = "megaphone_messages_received";
pub const MESSAGES_SENT_METRIC_NAME: &str = "megaphone_messages_read";
pub const MESSAGES_UNROUTABLE_METRIC_NAME: &str = "megaphone_messages_unroutable";
pub const MESSAGES_LOST_METRIC_NAME: &str = "megaphone_messages_lost";

pub struct BufferedChannel<Event> {
    full_id: String,
    tx: Sender<Event>,
    rx: Arc<Mutex<Receiver<Event>>>,
    last_read: Arc<Mutex<SystemTime>>,
    created_ts: Arc<Mutex<SystemTime>>,
}
const EVT_BUFFER_SIZE: usize = 100;

impl<Event> BufferedChannel<Event> {
    fn new(full_id: &str) -> Self {
        let (tx, rx) = channel(EVT_BUFFER_SIZE);
        Self {
            full_id: String::from(full_id),
            tx,
            rx: Arc::new(Mutex::new(rx)),
            last_read: Arc::new(Mutex::new(SystemTime::now())),
            created_ts: Arc::new(Mutex::new(SystemTime::now())),
        }
    }
}

impl<Event> Drop for BufferedChannel<Event> {
    fn drop(&mut self) {
        counter!(CHANNEL_DISPOSED_METRIC_NAME).increment(1);
        if let Ok(created) = self.created_ts.try_lock() {
            if let Ok(duration) = SystemTime::now().duration_since(*created) {
                histogram!(CHANNEL_DURATION_METRIC_NAME).record(duration.as_secs_f64());
            }
        } else {
            log::warn!("Could not lock created timestamp during channel dispose");
        }

        if let Ok(mut stream) = self.rx.try_lock() {
            while let Ok(_msg) = stream.try_recv() {
                counter!(MESSAGES_LOST_METRIC_NAME).increment(1);
            }
        } else {
            log::warn!("Could not lock receiver during channel dispose");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChannelShortId(pub u128);
impl ChannelShortId {
    pub fn from_id_segment(s: &str) -> Self {
        let hash = md5::compute(s.as_bytes());
        let short_id = u128::from_be_bytes(hash.0);
        Self(short_id)
    }

    pub fn from_full_id(id: &str) -> Result<Self, MegaphoneError> {
        let channel_uid = id
            .split('.')
            .nth(1)
            .ok_or_else(|| MegaphoneError::BadRequest(format!("Malformed channel id '{id}'")))?;

        Ok(Self::from_id_segment(channel_uid))
    }
}

pub struct MegaphoneService<MessageData> {
    webhooks: HashMap<String, WebHook>,
    agents_manager: AgentsManagerService,
    buffer: Arc<DashMap<ChannelShortId, BufferedChannel<MessageData>>>,
}

impl<Evt> Clone for MegaphoneService<Evt> {
    fn clone(&self) -> Self {
        Self {
            webhooks: self.webhooks.clone(),
            agents_manager: self.agents_manager.clone(),
            buffer: self.buffer.clone(),
        }
    }
}

impl<Event> MegaphoneService<Event> {
    pub fn new(webhooks: HashMap<String, WebHook>, agents_manager: AgentsManagerService) -> Self {
        Self {
            webhooks,
            agents_manager,
            buffer: Default::default(),
        }
    }

    pub async fn create_channel(
        &self,
        supported_protocols: &[String],
    ) -> Result<(String, String, String, Vec<String>), MegaphoneError> {
        if !supported_protocols.is_empty()
            && !supported_protocols.contains(&String::from(protocols::HTTP_STREAM_NDJSON_V1))
        {
            return Err(MegaphoneError::BadRequest(format!(
                "protocol(s) {:?} are not supported",
                supported_protocols
            )));
        }
        let vagent_id = self.agents_manager.random_master_id()?.to_string();

        let (channel_short_id, channel_full_id) = loop {
            let channel_id: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(50)
                .map(char::from)
                .collect();

            let short_id = ChannelShortId::from_id_segment(&channel_id);

            if !self.buffer.contains_key(&short_id) {
                break (short_id, channel_id);
            }
        };

        counter!(CHANNEL_CREATED_METRIC_NAME).increment(1);

        let full_id = format!(
            "{vagent_id}.{channel_full_id}.{}",
            Feature::new(megaphone::model::constants::features::CHAN_CHUNKED_STREAM).serialize()
        );
        let write_id = format!(
            "{vagent_id}.{}",
            self.agents_manager
                .encrypt_channel_id(&vagent_id, channel_short_id)?
        );

        self.buffer
            .insert(channel_short_id, BufferedChannel::new(&full_id));
        Ok((
            vagent_id,
            full_id,
            write_id,
            vec![String::from(protocols::HTTP_STREAM_NDJSON_V1)],
        ))
    }

    pub async fn create_channel_with_id(&self, id: &str) -> Result<(), MegaphoneError> {
        counter!(CHANNEL_CREATED_METRIC_NAME).increment(1);
        self.buffer
            .insert(ChannelShortId::from_full_id(id)?, BufferedChannel::new(id));
        Ok(())
    }

    pub async fn read_channel(
        &self,
        id: String,
        timeout: Duration,
    ) -> Result<impl futures::stream::Stream<Item = Event>, MegaphoneError> {
        let deadline = Instant::now() + timeout;
        let Some(channel) = self.buffer.get(&ChannelShortId::from_full_id(&id)?) else {
            return Err(MegaphoneError::NotFound);
        };
        if channel.full_id.ne(&id) {
            log::warn!(
                "Short-id matches but full id doesn't. Given '{id}' found '{}'",
                channel.full_id
            );
            return Err(MegaphoneError::NotFound);
        }
        let Ok(rx_guard) = channel.rx.clone().try_lock_owned() else {
            log::error!("rx mutex already locked");
            return Err(MegaphoneError::Busy);
        };
        let Ok(ts_guard) = channel.last_read.clone().try_lock_owned() else {
            log::error!("timestamp mutex already locked");
            return Err(MegaphoneError::Busy);
        };
        Ok(futures::stream::unfold(
            (rx_guard, ts_guard),
            move |(mut rx_guard, mut ts_guard)| async move {
                let next = tokio::time::timeout_at(deadline, rx_guard.recv()).await;
                match next {
                    Ok(Some(msg)) => {
                        counter!(MESSAGES_SENT_METRIC_NAME).increment(1);
                        Some((msg, (rx_guard, ts_guard)))
                    }
                    Ok(None) | Err(_) => {
                        *ts_guard = SystemTime::now();
                        None
                    }
                }
            },
        ))
    }

    pub fn channel_exists(&self, id: &str) -> bool {
        match self.parse_full_id(id) {
            Ok(channel_id) => self.buffer.contains_key(&channel_id),
            Err(err) => {
                log::warn!("Error parsing channel id '{id}' - {err}");
                false
            }
        }
    }

    pub fn drop_expired(&self) {
        let mut deleted_channels = Vec::new();
        self.buffer.retain(|_channel_id, channel| {
            let channel_not_expired = channel
                .last_read
                .try_lock()
                .map(|last_read| {
                    let deadline = SystemTime::now() - Duration::from_secs(60);
                    last_read.ge(&deadline)
                })
                .unwrap_or(true);

            let keep_channel = channel_not_expired
                || channel
                    .full_id
                    .split('.')
                    .next()
                    .and_then(|agent_id| self.agents_manager.is_agent_distributed(agent_id).ok())
                    .unwrap_or_else(|| {
                        log::warn!(
                            "Could not determine if agent is distributed for channel '{}'",
                            channel.full_id
                        );
                        false
                    });

            if !keep_channel {
                deleted_channels.push(channel.full_id.clone());
            }

            keep_channel
        });
        self.on_channels_deleted(deleted_channels);
    }

    fn on_channels_deleted(&self, deleted_channels: Vec<String>) {
        self.webhooks
            .iter()
            .filter(|(_, webhook)| matches!(webhook.hook, WebHookType::OnChannelDeleted))
            .for_each(|(name, webhook)| {
                let name = name.clone();
                let url = webhook.endpoint.clone();
                let body = json!({
                    "channels": deleted_channels,
                });

                tokio::spawn(async move {
                    let client = reqwest::Client::new();

                    let response = client.post(url).json(&body).send().await;

                    match response {
                        Err(err) => log::error!("Error processing webhook '{name}' - {err}"),
                        Ok(response) => {
                            if !response.status().is_success() {
                                log::error!(
                                    "Error processing webhook '{name}' - {}",
                                    response.status()
                                );
                            }
                        }
                    }
                });
            });
    }

    pub fn drop_channel(&self, id: &str) -> Result<(), MegaphoneError> {
        match self.parse_full_id(id) {
            Ok(channel_id) => {
                let Some((_id, _channel)) = self.buffer.remove(&channel_id) else {
                    return Err(MegaphoneError::InternalError(format!(
                        "Could not find channel with id {id}"
                    )));
                };
                Ok(())
            }
            Err(err) => {
                log::warn!("Error parsing channel id '{id}' - {err}");
                Err(MegaphoneError::BadRequest(format!(
                    "Malformed channel id '{id}'"
                )))
            }
        }
    }

    pub fn channel_ids_by_agent<'a>(&'a self, name: &str) -> impl Iterator<Item = String> + 'a {
        let agent_prefix = format!("{name}.");
        self.buffer
            .iter()
            .filter(move |channel| channel.full_id.starts_with(&agent_prefix))
            .map(|channel| channel.full_id.to_string())
    }

    pub fn list_channels<'a, C>(&'a self, skip: usize, limit: usize) -> anyhow::Result<Vec<C>>
    where
        Event: 'a,
        C: FromStr<Err = anyhow::Error>,
    {
        self.buffer
            .iter()
            .map(|v| v.full_id.parse::<C>())
            .skip(skip)
            .take(limit)
            .collect::<Result<_, _>>()
    }

    pub fn count_by_agent(&self, agent: &str) -> usize {
        let prefix = format!("{agent}.");
        self.buffer
            .iter()
            .filter(|entry| entry.full_id.starts_with(&prefix))
            .count()
    }

    fn parse_full_id(&self, full_id: &str) -> Result<ChannelShortId, MegaphoneError> {
        let mut fragments = full_id.split('.');
        let channel_id = fragments
            .next()
            .and_then(|agent_id| fragments.next().map(|channel_id| (agent_id, channel_id)))
            .filter(|(_, channel_id)| channel_id.len() != 50)
            .map(|(agent_id, channel_id)| {
                self.agents_manager.decrypt_channel_id(agent_id, channel_id)
            })
            .unwrap_or_else(|| ChannelShortId::from_full_id(full_id))?;
        Ok(channel_id)
    }
}

pub trait WithTimestamp {
    fn timestamp(&self) -> SystemTime;
}

impl WithTimestamp for EventDto {
    fn timestamp(&self) -> SystemTime {
        self.timestamp.into()
    }
}

impl MegaphoneService<EventDto> {
    pub async fn write_batch_into_channels(
        &self,
        ids: &[impl AsRef<str>],
        messages: Vec<EventDto>,
    ) -> Vec<MessageDeliveryFailure> {
        let results_fut = ids.iter().map(|chan_id| {
            self.write_batch_into_channel(chan_id.as_ref(), messages.clone())
                .map(|res| (chan_id.as_ref(), res))
        });

        let results = futures::future::join_all(results_fut).await;

        results
            .into_iter()
            .flat_map(|(chan_id, results)| {
                results
                    .into_iter()
                    .enumerate()
                    .flat_map(|(idx, res)| res.err().map(|err| (idx, err)))
                    .map(|(index, err)| MessageDeliveryFailure {
                        channel: chan_id.to_string(),
                        index,
                        reason: String::from(err.code()),
                    })
            })
            .collect()
    }

    async fn write_batch_into_channel(
        &self,
        id: &str,
        messages: Vec<EventDto>,
    ) -> Vec<Result<(), MegaphoneError>> {
        let mut results = Vec::with_capacity(messages.len());
        let mut timeout_reached = false;
        for message in messages {
            if !timeout_reached {
                let result = self.write_into_channel(id, message).await;
                if let Err(MegaphoneError::Timeout { .. }) = &result {
                    timeout_reached = true;
                }
                results.push(result);
            } else {
                results.push(Err(MegaphoneError::Skipped));
            }
        }
        results
    }

    pub async fn write_into_channel(
        &self,
        full_id: &str,
        message: EventDto,
    ) -> Result<(), MegaphoneError> {
        let channel_id = self.parse_full_id(full_id)?;

        let Some(channel) = self.buffer.get(&channel_id) else {
            counter!(MESSAGES_UNROUTABLE_METRIC_NAME).increment(1);
            return Err(MegaphoneError::NotFound);
        };
        counter!(MESSAGES_RECEIVED_METRIC_NAME).increment(1);

        let pipes = channel
            .full_id
            .split('.')
            .next()
            .map(|agent_id| self.agents_manager.get_pipes(agent_id))
            .unwrap_or_default();

        for pipe in &pipes {
            let out = pipe.try_send(SyncEvent::EventReceived {
                channel: full_id.to_string(),
                event: message.clone(),
            });
            if let Err(err) = out {
                log::error!("Error during event pipe - {err}");
            }
        }

        if !pipes.is_empty() {
            match channel.tx.try_send(message) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(message)) => channel.force_write(message),
                Err(TrySendError::Closed(_)) => Err(MegaphoneError::InternalError(String::from(
                    "Channel is closed",
                ))),
            }
        } else {
            let tx = channel.tx.clone();
            drop(channel);
            tx.send_timeout(message, Duration::from_secs(10))
                .await
                .map_err(|err| match err {
                    SendTimeoutError::Timeout(_) => MegaphoneError::Timeout { secs: 10 },
                    SendTimeoutError::Closed(_) => {
                        MegaphoneError::InternalError(String::from("Channel is closed"))
                    }
                })
        }
    }

    pub fn inject_into_channel(&self, id: &str, message: EventDto) -> Result<(), MegaphoneError> {
        let Some(channel) = self.buffer.get(&ChannelShortId::from_full_id(id)?) else {
            counter!(MESSAGES_UNROUTABLE_METRIC_NAME).increment(1);
            return Err(MegaphoneError::NotFound);
        };
        counter!(MESSAGES_RECEIVED_METRIC_NAME).increment(1);
        match channel.tx.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(message)) => channel.force_write(message),
            Err(TrySendError::Closed(_message)) => {
                log::error!("Error injecting message - channel is disconnected");
                Err(MegaphoneError::InternalError(String::from(
                    "Disconnected channel",
                )))
            }
        }
    }
}

impl<Event: WithTimestamp> BufferedChannel<Event> {
    pub fn force_write(&self, event: Event) -> Result<(), MegaphoneError> {
        let mut rx = self.rx.try_lock().map_err(|_err| {
            MegaphoneError::InternalError(String::from("Cannot lock channel rx"))
        })?;
        let mut buffered_evts = Vec::with_capacity(EVT_BUFFER_SIZE);
        let now = SystemTime::now();
        let _skipped = rx.try_recv();
        // Skip first event to preserve one slot
        counter!(MESSAGES_LOST_METRIC_NAME).increment(1);
        while let Ok(evt) = rx.try_recv() {
            if evt.timestamp().add(Duration::from_secs(60)).gt(&now) {
                buffered_evts.push(evt);
            } else {
                counter!(MESSAGES_LOST_METRIC_NAME).increment(1);
            }
        }
        buffered_evts.push(event);
        for evt in buffered_evts {
            let out = self.tx.try_send(evt);
            if let Err(err) = out {
                log::error!("Error writing back event to the channel - {err}");
            }
        }
        Ok(())
    }
}
