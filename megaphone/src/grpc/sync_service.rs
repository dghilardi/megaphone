use std::collections::HashSet;
use futures::StreamExt;
use tonic::{Request, Response, Status, Streaming};
use crate::dto::message::EventDto;

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
                    } else if let Some(agent) = self.agent_mgr.list_agents().find(|agent| agent.key().eq(&req.agent_id)) {
                        log::warn!("agent-id {} is already registered: {:?}", &agent.key(), agent.value())
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
                    } else if let Some(agent) = self.agent_mgr.list_agents().find(|agent| agent.key().eq(&req.agent_id)) {
                        let out = self.agent_mgr.close_replica_session(&req.agent_id);
                        if let Err(err) = out {
                            log::error!("Error closing pipe session - {err}");
                        }
                    } else {
                        log::warn!("agent-id {} is not registered", req.agent_id);
                    }
                }
                Ok(SyncRequest { sync_event: Some(SyncEvent::ChannelDisposed(req)) }) => {}
                Ok(SyncRequest { sync_event: Some(SyncEvent::ChannelCreated(req)) }) => {
                    let out = self.megaphone_svc.create_channel_with_id(&req.channel_id).await;
                    if let Err(err) = out {
                        log::error!("Error processing channel-created - {err}");
                    }
                }
                Ok(SyncRequest { sync_event: Some(SyncEvent::EventReceived(req)) }) => {
                    let out = self.megaphone_svc.write_into_channel(&req.channel_id.clone(), EventDto::from(req)).await;
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

impl From<EventReceived> for EventDto {
    fn from(value: EventReceived) -> Self {
        Self {
            stream_id: value.stream_id,
            event_id: value.event_id,
            body: Default::default(),
        }
    }
}