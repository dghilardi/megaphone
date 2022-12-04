use std::sync::Arc;
use std::time::{Duration, SystemTime};

use dashmap::DashMap;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::Mutex;
use tokio::time::Instant;
use uuid::Uuid;
use crate::core::error::MegaphoneError;

pub struct BufferedChannel<Event> {
    tx: Sender<Event>,
    rx: Arc<Mutex<Receiver<Event>>>,
    last_read: Arc<Mutex<SystemTime>>,
}

impl<Event> BufferedChannel<Event> {
    fn new() -> Self {
        let (tx, rx) = channel(100);
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
            last_read: Arc::new(Mutex::new(SystemTime::now())),
        }
    }
}

pub struct MegaphoneService<MessageData> {
    buffer: Arc<DashMap<Uuid, BufferedChannel<MessageData>>>,
}

impl<Evt> Clone for MegaphoneService<Evt> {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
        }
    }
}

impl<Event> MegaphoneService<Event> {
    pub fn new() -> Self {
        Self { buffer: Default::default() }
    }

    pub async fn create_channel(&self) -> String {
        let uuid = Uuid::new_v4();
        self.buffer.insert(uuid, BufferedChannel::new());
        uuid.to_string()
    }

    pub async fn read_channel(&self, id: Uuid, timeout: Duration) -> impl futures::stream::Stream<Item=Event> {
        let deadline = Instant::now() + timeout;
        let Some(channel) = self.buffer.get(&id) else {
            panic!("handle channel not found");
        };
        let Ok(rx_guard) = channel.rx.clone().try_lock_owned() else {
            panic!("mutex already locked");
        };
        let Ok(ts_guard) = channel.last_read.clone().try_lock_owned() else {
            panic!("timestamp mutex already locked");
        };
        futures::stream::unfold((rx_guard, ts_guard), move |(mut rx_guard, mut ts_guard)| async move {
            let next = tokio::time::timeout_at(deadline, rx_guard.recv()).await;
            match next {
                Ok(Some(msg)) => Some((msg, (rx_guard, ts_guard))),
                Ok(None) | Err(_) => {
                    *ts_guard = SystemTime::now();
                    None
                }
            }
        })
    }

    pub async fn write_into_channel(&self, id: Uuid, message: Event) -> Result<(), MegaphoneError> {
        let Some(channel) = self.buffer.get(&id) else {
            return Err(MegaphoneError::NotFound);
        };
        channel.tx
            .send(message)
            .await
            .map_err(|_| MegaphoneError::InternalError)
    }

    pub fn drop_expired(&self) {
        self.buffer
            .retain(|_, channel|
                channel.last_read
                    .try_lock()
                    .map(|last_read| {
                        let deadline = SystemTime::now() - Duration::from_secs(60);
                        last_read.ge(&deadline)
                    })
                    .unwrap_or(true)
            );
    }
}