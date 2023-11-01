use std::ops::Add;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use dashmap::DashMap;
use metrics::{histogram, increment_counter};
use rand::distributions::Alphanumeric;
use rand::Rng;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::core::error::MegaphoneError;
use crate::dto::message::EventDto;
use crate::service::agents_manager_service::{AgentsManagerService, SyncEvent, VirtualAgentStatus};

pub const CHANNEL_CREATED_METRIC_NAME: &str = "megaphone_channel_created";
pub const CHANNEL_DISPOSED_METRIC_NAME: &str = "megaphone_channel_disposed";
pub const CHANNEL_DURATION_METRIC_NAME: &str = "megaphone_channel_duration";
pub const MESSAGES_RECEIVED_METRIC_NAME: &str = "megaphone_messages_received";
pub const MESSAGES_SENT_METRIC_NAME: &str = "megaphone_messages_read";
pub const MESSAGES_UNROUTABLE_METRIC_NAME: &str = "megaphone_messages_unroutable";
pub const MESSAGES_LOST_METRIC_NAME: &str = "megaphone_messages_lost";

pub struct BufferedChannel<Event> {
    tx: Sender<Event>,
    rx: Arc<Mutex<Receiver<Event>>>,
    last_read: Arc<Mutex<SystemTime>>,
    created_ts: Arc<Mutex<SystemTime>>,
}

const EVT_BUFFER_SIZE: usize = 100;

impl<Event> BufferedChannel<Event> {
    fn new() -> Self {
        let (tx, rx) = channel(EVT_BUFFER_SIZE);
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
            last_read: Arc::new(Mutex::new(SystemTime::now())),
            created_ts: Arc::new(Mutex::new(SystemTime::now())),
        }
    }
}

pub struct MegaphoneService<MessageData> {
    agents_manager: AgentsManagerService,
    buffer: Arc<DashMap<String, BufferedChannel<MessageData>>>,
}

impl<Evt> Clone for MegaphoneService<Evt> {
    fn clone(&self) -> Self {
        Self {
            agents_manager: self.agents_manager.clone(),
            buffer: self.buffer.clone(),
        }
    }
}

impl<Event> MegaphoneService<Event> {
    pub fn new(
        agents_manager: AgentsManagerService,
    ) -> Self {
        Self { agents_manager, buffer: Default::default() }
    }

    pub async fn create_channel(&self) -> Result<(String, String), MegaphoneError> {
        let vagent_id = self.agents_manager.random_master_id()?.to_string();

        let channel_id: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(50)
            .map(char::from)
            .collect();

        increment_counter!(CHANNEL_CREATED_METRIC_NAME);

        let full_id = format!("{vagent_id}.{channel_id}");

        self.buffer.insert(full_id.clone(), BufferedChannel::new());
        Ok((vagent_id, full_id))
    }

    pub async fn create_channel_with_id(&self, id: &str) -> Result<(), MegaphoneError> {
        increment_counter!(CHANNEL_CREATED_METRIC_NAME);
        self.buffer.insert(id.to_string(), BufferedChannel::new());
        Ok(())
    }

    pub async fn read_channel(&self, id: String, timeout: Duration) -> Result<impl futures::stream::Stream<Item=Event>, MegaphoneError> {
        let deadline = Instant::now() + timeout;
        let Some(channel) = self.buffer.get(&id) else {
            return Err(MegaphoneError::NotFound);
        };
        let Ok(rx_guard) = channel.rx.clone().try_lock_owned() else {
            log::error!("rx mutex already locked");
            return Err(MegaphoneError::Busy);
        };
        let Ok(ts_guard) = channel.last_read.clone().try_lock_owned() else {
            log::error!("timestamp mutex already locked");
            return Err(MegaphoneError::Busy);
        };
        Ok(futures::stream::unfold((rx_guard, ts_guard), move |(mut rx_guard, mut ts_guard)| async move {
            let next = tokio::time::timeout_at(deadline, rx_guard.recv()).await;
            match next {
                Ok(Some(msg)) => {
                    increment_counter!(MESSAGES_SENT_METRIC_NAME);
                    Some((msg, (rx_guard, ts_guard)))
                },
                Ok(None) | Err(_) => {
                    *ts_guard = SystemTime::now();
                    None
                }
            }
        }))
    }

    pub fn channel_exists(&self, id: &str) -> bool {
        self.buffer.contains_key(id)
    }

    pub fn drop_expired(&self) {
        self.buffer
            .retain(|channel_id, channel| {
                let channel_not_expired = channel.last_read
                    .try_lock()
                    .map(|last_read| {
                        let deadline = SystemTime::now() - Duration::from_secs(60);
                        last_read.ge(&deadline)
                    })
                    .unwrap_or(true);

                let keep_channel = !channel_not_expired || channel_id.split('.').next()
                    .and_then(|agent_id| self.agents_manager.is_agent_distributed(agent_id).ok())
                    .unwrap_or_else(|| {
                        log::warn!("Could not determine if agent is distributed");
                        false
                    });

                if !keep_channel {
                    increment_counter!(CHANNEL_DISPOSED_METRIC_NAME);
                    if let Ok(created) = channel.created_ts.try_lock() {
                        if let Ok(duration) = SystemTime::now().duration_since(*created) {
                            histogram!(CHANNEL_DURATION_METRIC_NAME, duration.as_secs_f64());
                        }
                    } else {
                        log::warn!("Could not lock created timestamp during channel dispose");
                    }

                    if let Ok(mut stream) = channel.rx.try_lock() {
                        while let Ok(msg) = stream.try_recv() {
                            increment_counter!(MESSAGES_LOST_METRIC_NAME);
                        }
                    } else {
                        log::warn!("Could not lock receiver during channel dispose");
                    }
                }

                channel_not_expired
            });
    }

    pub fn channel_ids_by_agent<'a>(&'a self, name: &str) -> impl Iterator<Item=String> + 'a {
        let agent_prefix = format!("{name}.");
        self.buffer.iter()
            .filter(move |channel| channel.key().starts_with(&agent_prefix))
            .map(|channel| channel.key().to_string())
    }
}

pub trait WithTimestamp {
    fn timestamp(&self) -> SystemTime;
}

impl MegaphoneService<EventDto> {

    pub async fn write_into_channel(&self, id: &str, message: EventDto) -> Result<(), MegaphoneError> {
        let Some(channel) = self.buffer.get(id) else {
            increment_counter!(MESSAGES_UNROUTABLE_METRIC_NAME);
            return Err(MegaphoneError::NotFound);
        };
        increment_counter!(MESSAGES_RECEIVED_METRIC_NAME);

        let pipes = id.split('.').next()
            .map(|agent_id| self.agents_manager.get_pipes(agent_id))
            .unwrap_or_default();

        for pipe in &pipes {
            let out = pipe.try_send(SyncEvent::EventReceived { channel: id.to_string(), event: message.clone() });
            if let Err(err) = out {
                log::error!("Error during event pipe - {err}");
            }
        }

        if !pipes.is_empty() {
            match channel.tx.try_send(message) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(message)) => {
                    channel.force_write(message)
                },
                Err(TrySendError::Closed(_)) => Err(MegaphoneError::InternalError(format!("Channel is closed"))),
            }
        } else {
            channel.tx
                .send(message)
                .await
                .map_err(|err| MegaphoneError::InternalError(format!("Error writing channel - {err}")))
        }
    }

    pub fn inject_into_channel(&self, id: &str, message: EventDto) -> Result<(), MegaphoneError> {
        let Some(channel) = self.buffer.get(id) else {
            increment_counter!(MESSAGES_UNROUTABLE_METRIC_NAME);
            return Err(MegaphoneError::NotFound);
        };
        increment_counter!(MESSAGES_RECEIVED_METRIC_NAME);
        match channel.tx.try_send(message) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(message)) => {
                channel.force_write(message)
            },
            Err(TrySendError::Closed(message)) => {
                log::error!("Error injecting message - channel is disconnected");
                Err(MegaphoneError::InternalError(String::from("Disconnected channel")))
            }
        }
    }
}

impl <Event: WithTimestamp> BufferedChannel<Event> {
    pub fn force_write(&self, event: Event) -> Result<(), MegaphoneError> {
        let mut rx = self.rx.try_lock()
            .map_err(|_err| MegaphoneError::InternalError(String::from("Cannot lock channel rx")))?;
        let mut buffered_evts = Vec::with_capacity(EVT_BUFFER_SIZE);
        let now = SystemTime::now();
        let _skipped = rx.try_recv(); // Skip first event to preserve one slot
        increment_counter!(MESSAGES_LOST_METRIC_NAME);
        while let Ok(evt) = rx.try_recv() {
            if evt.timestamp().add(Duration::from_secs(60)).gt(&now) {
                buffered_evts.push(evt);
            } else {
                increment_counter!(MESSAGES_LOST_METRIC_NAME);
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