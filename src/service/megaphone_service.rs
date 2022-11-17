use std::io;
use std::sync::Arc;
use axum::BoxError;
use axum::response::ErrorResponse;
use dashmap::DashMap;
use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio::sync::Mutex;
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

    pub async fn read_channel(&self, id: Uuid) -> impl futures::stream::Stream<Item=Result<String, BoxError>> {
        let Some(channel) = self.buffer.get(&id) else {
            panic!("handle channel not found");
        };
        futures::stream::unfold(channel.rx.clone(), |rx| async {
            let mut guard = rx.lock().await;
            let maybe_next = guard.recv().await;
            drop(guard);
            maybe_next.map(|next| (Ok(next), rx))
        })
    }
}