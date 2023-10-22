use std::sync::Arc;
use std::time::{Duration, SystemTime};

use dashmap::DashMap;
use metrics::{histogram, increment_counter};
use rand::distributions::Alphanumeric;
use rand::Rng;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::core::error::MegaphoneError;
use crate::service::agents_manager_service::AgentsManagerService;

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

impl<Event> BufferedChannel<Event> {
    fn new() -> Self {
        let (tx, rx) = channel(100);
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

    pub async fn create_channel(&self) -> (String, String) {
        let vagent_id = self.agents_manager.random_master_id().to_string();

        let channel_id: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(50)
            .map(char::from)
            .collect();

        increment_counter!(CHANNEL_CREATED_METRIC_NAME);

        let full_id = format!("{vagent_id}.{channel_id}");

        self.buffer.insert(full_id.clone(), BufferedChannel::new());
        (vagent_id, full_id)
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

    pub async fn write_into_channel(&self, id: String, message: Event) -> Result<(), MegaphoneError> {
        let Some(channel) = self.buffer.get(&id) else {
            increment_counter!(MESSAGES_UNROUTABLE_METRIC_NAME);
            return Err(MegaphoneError::NotFound);
        };
        increment_counter!(MESSAGES_RECEIVED_METRIC_NAME);
        channel.tx
            .send(message)
            .await
            .map_err(|_| MegaphoneError::InternalError)
    }

    pub fn channel_exists(&self, id: &str) -> bool {
        self.buffer.contains_key(id)
    }

    pub fn drop_expired(&self) {
        self.buffer
            .retain(|_, channel| {
                let retain_item = channel.last_read
                    .try_lock()
                    .map(|last_read| {
                        let deadline = SystemTime::now() - Duration::from_secs(60);
                        last_read.ge(&deadline)
                    })
                    .unwrap_or(true);

                if !retain_item {
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

                retain_item
            });
    }
}