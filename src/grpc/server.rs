use chrono::{DateTime, Utc};
use prost_types::Timestamp;

use crate::grpc::server::megaphone::SyncRequest;
use crate::service::agents_manager_service::SyncEvent;

pub mod megaphone {
    tonic::include_proto!("megaphone"); // The string specified here must match the proto package name
}

impl From<SyncEvent> for SyncRequest {
    fn from(value: SyncEvent) -> Self {
        Self { sync_event: Some(From::from(value)) }
    }
}

impl From<SyncEvent> for megaphone::sync_request::SyncEvent {
    fn from(value: SyncEvent) -> Self {
        match value {
            SyncEvent::PipeAgentStart { name, key } => Self::PipeAgentStart(megaphone::PipeAgentStart { agent_id: name, key: key.to_vec() }),
            SyncEvent::PipeAgentEnd { name } => Self::PipeAgentEnd(megaphone::PipeAgentEnd { agent_id: name }),
            SyncEvent::ChannelCreated { id } => Self::ChannelCreated(megaphone::ChannelCreated { channel_id: id }),
            SyncEvent::ChannelDisposed { id } => Self::ChannelDisposed(megaphone::ChannelDisposed { channel_id: id }),
            SyncEvent::EventReceived { channel, event } => Self::EventReceived(megaphone::EventReceived {
                channel_id: channel,
                stream_id: event.stream_id,
                event_id: event.event_id,
                timestamp: Some(datetime_to_timestamp(event.timestamp)),
                json_payload: serde_json::to_string(&event.body).expect("Error serializing payload"),
            }),
        }
    }
}

fn datetime_to_timestamp(datetime: DateTime<Utc>) -> Timestamp {
    Timestamp {
        seconds: datetime.timestamp(),
        nanos: datetime.timestamp_subsec_nanos().try_into().unwrap_or(i32::MAX),
    }
}