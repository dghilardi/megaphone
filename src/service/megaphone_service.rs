use std::io;
use std::sync::Arc;
use std::time::Duration;
use axum::BoxError;
use axum::response::ErrorResponse;
use dashmap::DashMap;
use futures::{FutureExt, select};
use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio::sync::Mutex;
use tokio::time::Instant;
use uuid::Uuid;

type MessageData = String;

pub struct BufferedChannel {
    tx: Sender<MessageData>,
    rx: Arc<Mutex<Receiver<MessageData>>>,
}

impl BufferedChannel {
    fn new() -> Self {
        let (tx, rx) = channel(100);
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }
}

pub struct MegaphoneService {
    buffer: DashMap<Uuid, BufferedChannel>
}

impl MegaphoneService {
    pub fn new() -> Self {
        Self { buffer: Default::default() }
    }

    pub async fn create_channel(&self) -> String {
        let uuid = Uuid::new_v4();
        self.buffer.insert(uuid, BufferedChannel::new());
        uuid.to_string()
    }

    pub async fn read_channel(&self, id: Uuid, timeout: Duration) -> impl futures::stream::Stream<Item=Result<String, BoxError>> {
        let deadline = Instant::now() + timeout;
        let Some(channel) = self.buffer.get(&id) else {
            panic!("handle channel not found");
        };
        futures::stream::unfold(channel.rx.clone(), move |rx| async move {
            let mut guard = rx.lock().await;
            let next = tokio::time::timeout_at(deadline, guard.recv()).await;
            drop(guard);
            match next {
                Ok(Some(msg)) => Some((Ok(msg), rx)),
                Ok(None) => None,
                Err(_) => None,
            }
        })
    }
}