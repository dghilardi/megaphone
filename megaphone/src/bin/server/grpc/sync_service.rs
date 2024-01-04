use std::cmp;
use std::collections::HashSet;

use chrono::{DateTime, NaiveDateTime, Utc};
use futures::StreamExt;
use prost_types::Timestamp;
use tonic::{Request, Response, Status, Streaming};

use megaphone::dto::message::EventDto;

use crate::core::error::MegaphoneError;
use crate::grpc::server::megaphone::{EventReceived, SyncReply, SyncRequest};
use crate::grpc::server::megaphone::sync_request::SyncEvent;
use crate::grpc::server::megaphone::sync_service_server::SyncService;
use crate::service::agents_manager_service::AgentsManagerService;
use crate::service::megaphone_service::MegaphoneService;

pub struct MegaphoneSyncService {
    agent_mgr: AgentsManagerService,
    megaphone_svc: MegaphoneService<EventDto>,
}

impl MegaphoneSyncService {
    pub fn new(
        agent_mgr: AgentsManagerService,
        megaphone_svc: MegaphoneService<EventDto>,
    ) -> Self {
        Self {
            agent_mgr,
            megaphone_svc,
        }
    }
}

#[tonic::async_trait]
impl SyncService for MegaphoneSyncService {
    async fn forward_events(&self, request: Request<Streaming<SyncRequest>>) -> Result<Response<SyncReply>, Status> {
        let mut stream = request.into_inner();
        let mut piped_agents = HashSet::new();
        while let Some(stream_item) = stream.next().await {
            match stream_item {
                Ok(SyncRequest { sync_event: Some(SyncEvent::PipeAgentStart(req)) }) => {
                    if piped_agents.contains(&req.agent_id) {
                        log::warn!("agent-id {} is already piped by this session", req.agent_id);
                    } else if let Some((name, props)) = self.agent_mgr.find_agent(&req.agent_id) {
                        log::warn!("agent-id {name} is already registered: {props:?}")
                    } else {
                        let out = self.agent_mgr.open_replica_session(&req.agent_id);
                        if let Err(err) = out {
                            log::error!("Error opening pipe session - {err}");
                        } else {
                            piped_agents.insert(req.agent_id);
                        }
                    }
                }
                Ok(SyncRequest { sync_event: Some(SyncEvent::PipeAgentEnd(req)) }) => {
                    if !piped_agents.remove(&req.agent_id) {
                        log::warn!("agent-id {} was not piped by this session", req.agent_id);
                    } else if let Some((_name, _props)) = self.agent_mgr.find_agent(&req.agent_id) {
                        let out = self.agent_mgr.close_replica_session(&req.agent_id);
                        if let Err(err) = out {
                            log::error!("Error closing pipe session - {err}");
                        }
                    } else {
                        log::warn!("agent-id {} is not registered", req.agent_id);
                    }
                }
                Ok(SyncRequest { sync_event: Some(SyncEvent::ChannelDisposed(_req)) }) => {}
                Ok(SyncRequest { sync_event: Some(SyncEvent::ChannelCreated(req)) }) => {
                    let out = self.megaphone_svc.create_channel_with_id(&req.channel_id).await;
                    if let Err(err) = out {
                        log::error!("Error processing channel-created - {err}");
                    }
                }
                Ok(SyncRequest { sync_event: Some(SyncEvent::EventReceived(req)) }) => {
                    let channel_id = req.channel_id.clone();
                    let out = EventDto::try_from(req)
                        .and_then(|evt| self.megaphone_svc.inject_into_channel(&channel_id, evt));
                    if let Err(err) = out {
                        log::error!("Error processing event-received - {err}");
                    }
                },
                Ok(SyncRequest { sync_event: None }) => log::warn!("Received grpc SyncRequest without sync_event"),
                Err(err) => log::warn!("Error in grpc SyncRequest - {err}"),
            }
        }

        for agent in piped_agents {
            let out = self.agent_mgr.close_replica_session(&agent);
            if let Err(err) = out {
                log::error!("Error closing pipe session - {err}");
            }
        }
        Ok(Response::new(SyncReply { message: String::from("OK") }))
    }
}

fn datetime_to_timestamp(datetime: DateTime<Utc>) -> Timestamp {
    Timestamp {
        seconds: datetime.timestamp(),
        nanos: datetime.timestamp_subsec_nanos().try_into().unwrap_or(i32::MAX),
    }
}

fn timestamp_to_datetime(timestamp: Timestamp) -> Option<DateTime<Utc>> {
    let naive = NaiveDateTime::from_timestamp_opt(timestamp.seconds, cmp::max(0, timestamp.nanos) as u32)?;
    Some(DateTime::from_utc(naive, Utc))
}

impl TryFrom<EventReceived> for EventDto {
    type Error = MegaphoneError;

    fn try_from(value: EventReceived) -> Result<Self, Self::Error> {
        Ok(Self {
            stream_id: value.stream_id,
            event_id: value.event_id,
            timestamp: value.timestamp
                .and_then(timestamp_to_datetime)
                .unwrap_or_else(Utc::now),
            body: serde_json::from_str(&value.json_payload)
                .map_err(|err| MegaphoneError::BadRequest(format!("Cannot deserialize json payload - {err}")))?,
        })
    }
}