use std::sync::Arc;
use std::time::Duration;

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
}

impl <Event> BufferedChannel<Event> {
    fn new() -> Self {
        let (tx, rx) = channel(100);
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }
}

pub struct MegaphoneService<MessageData> {
    buffer: Arc<DashMap<Uuid, BufferedChannel<MessageData>>>
}

impl <Evt> Clone for MegaphoneService<Evt> {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
        }
    }
}

impl <Event> MegaphoneService<Event> {
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
        let Ok(guard) = channel.rx.clone().try_lock_owned() else {
            panic!("mutex already locked");
        };
        futures::stream::unfold(guard, move |mut guard| async move {
            let next = tokio::time::timeout_at(deadline, guard.recv()).await;
            match next {
                Ok(Some(msg)) => Some((msg, guard)),
                Ok(None) => None,
                Err(_) => None,
            }
        })
    }

    pub async fn write_into_channel(&self, id: Uuid, message: Event) -> Result<(), MegaphoneError> {
        let Some(channel) = self.buffer.get(&id) else {
            return Err(MegaphoneError::NotFound)
        };
        channel.tx
            .send(message)
            .await
            .map_err(|_| MegaphoneError::InternalError)
    }
}