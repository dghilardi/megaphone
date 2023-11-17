use std::collections::hash_map::RandomState;
use std::ops::Add;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use dashmap::DashMap;
use dashmap::mapref::multiple::RefMulti;
use lazy_static::lazy_static;
use rand::seq::IteratorRandom;
use tokio::sync::mpsc;

use crate::core::config::{AgentConfig, VirtualAgentMode};
use crate::core::error::MegaphoneError;
use crate::dto::message::EventDto;

pub const WARMUP_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct VirtualAgentProps {
    change_ts: SystemTime,
    status: VirtualAgentStatus,
}

impl VirtualAgentProps {
    pub fn new(mode: VirtualAgentStatus) -> Self {
        Self {
            change_ts: SystemTime::now(),
            status: mode,
        }
    }

    pub fn change_status(&mut self, status: VirtualAgentStatus) {
        self.change_ts = SystemTime::now();
        self.status = status;
    }

    pub fn status(&self) -> &VirtualAgentStatus {
        &self.status
    }

    pub fn change_ts(&self) -> SystemTime {
        self.change_ts
    }

    pub fn is_warming_up(&self) -> bool {
        match self.status {
            VirtualAgentStatus::Master => self.change_ts.add(Duration::from_secs(WARMUP_SECS)).gt(&SystemTime::now()),
            VirtualAgentStatus::Replica { .. } => false,
            VirtualAgentStatus::Piped { .. } => false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum VirtualAgentStatus {
    Master,
    Replica { pipe_sessions_count: usize },
    Piped { pipes: Vec<mpsc::Sender<SyncEvent>> },
}

impl From<VirtualAgentMode> for VirtualAgentStatus {
    fn from(value: VirtualAgentMode) -> Self {
        match value {
            VirtualAgentMode::Master => Self::Master,
            VirtualAgentMode::Replica => Self::Replica { pipe_sessions_count: 0 },
        }
    }
}

pub struct AgentsManagerService {
    virtual_agents: Arc<DashMap<String, VirtualAgentProps>>,
}

impl Clone for AgentsManagerService {
    fn clone(&self) -> Self {
        Self {
            virtual_agents: self.virtual_agents.clone(),
        }
    }
}

impl AgentsManagerService {
    pub fn new(conf: AgentConfig) -> Result<Self, MegaphoneError> {
        let virtual_agents = conf.virtual_agents
            .into_iter()
            .map(|(k, v)| Self::validate_agent_name(&k).map(|_| (k, VirtualAgentProps::new(v.into()))))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            virtual_agents: Arc::new(virtual_agents),
        })
    }

    fn validate_agent_name(name: &str) -> Result<(), MegaphoneError> {
        lazy_static! {
        static ref RE: regex::Regex = regex::Regex::new(r"^[A-Za-z0-9_\-]+$").unwrap();
    }
        if RE.is_match(name) {
            Ok(())
        } else {
            Err(MegaphoneError::BadRequest(format!("Unsupported format for agent name. Given '{name}'")))
        }
    }

    fn active_masters(&self) -> impl Iterator<Item=RefMulti<String, VirtualAgentProps>> {
        self.virtual_agents.iter()
            .filter(|entry| matches!(entry.value().status(), VirtualAgentStatus::Master))
            .filter(|entry| !entry.value().is_warming_up())
    }

    pub fn random_master_id(&self) -> Result<String, MegaphoneError> {
        self.active_masters()
            .map(|entry| entry.key().to_string())
            .choose(&mut rand::thread_rng())
            .ok_or(MegaphoneError::InternalError(String::from("No virtual agent with master status was found")))
    }

    pub fn list_agents(&self) -> Vec<(String, VirtualAgentProps)> {
        self.virtual_agents
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    pub fn find_agent(&self, name: &str) -> Option<(String, VirtualAgentProps)> {
        self.virtual_agents.iter()
            .find(|entry| entry.key().eq(name))
            .map(|entry| (entry.key().clone(), entry.value().clone()))
    }

    pub fn add_master(&self, name: &str) -> Result<(), MegaphoneError> {
        Self::validate_agent_name(name)?;
        self.virtual_agents.insert(String::from(name), VirtualAgentProps::new(VirtualAgentStatus::Master));
        Ok(())
    }

    pub fn open_replica_session(&self, name: &str) -> Result<(), MegaphoneError> {
        let entry = self.virtual_agents.entry(String::from(name))
            .or_insert_with(|| VirtualAgentProps::new(VirtualAgentStatus::Replica { pipe_sessions_count: 0 }));

        let VirtualAgentStatus::Replica { mut pipe_sessions_count } = entry.status else {
            return Err(MegaphoneError::InternalError(format!("{name} agent is already registered but is not a replica")));
        };
        pipe_sessions_count += 1;

        Ok(())
    }

    pub fn close_replica_session(&self, name: &str) -> Result<(), MegaphoneError> {
        let Some(entry) = self.virtual_agents.get(name) else {
            return Err(MegaphoneError::InternalError(format!("{name} agent is not registered")));
        };
        let VirtualAgentStatus::Replica { mut pipe_sessions_count } = entry.status else {
            return Err(MegaphoneError::InternalError(format!("{name} agent is already registered but is not a replica")));
        };
        pipe_sessions_count -= 1;
        Ok(())
    }

    pub fn is_agent_distributed(&self, name: &str) -> Result<bool, MegaphoneError> {
        let Some(agent) = self.virtual_agents.get(name) else {
            return Err(MegaphoneError::InternalError(format!("Agent {name} is not registered")));
        };
        match agent.status {
            VirtualAgentStatus::Master => Ok(false),
            VirtualAgentStatus::Replica { pipe_sessions_count: 0 } => Ok(false),
            VirtualAgentStatus::Replica { .. } => Ok(true),
            VirtualAgentStatus::Piped { .. } => Ok(true),
        }
    }

    pub fn get_pipes(&self, name: &str) -> Vec<mpsc::Sender<SyncEvent>> {
        self.virtual_agents.get(name)
            .and_then(|agent| if let VirtualAgentStatus::Piped { pipes } = &agent.status { Some(pipes.clone()) } else { None })
            .unwrap_or_default()
    }

    pub fn register_pipe(&self, name: &str, pipe: mpsc::Sender<SyncEvent>) -> Result<(), MegaphoneError> {
        let Some(mut agent) = self.virtual_agents.get_mut(name) else {
            return Err(MegaphoneError::BadRequest(format!("Agent {name} is not registered")))
        };
        let Ok(()) = pipe.try_send(SyncEvent::PipeAgentStart { name: name.to_string() }) else {
            return Err(MegaphoneError::InternalError(format!("Error sending pipe registration event for agent {name}")))
        };
        agent.status = match &agent.status {
            VirtualAgentStatus::Master => VirtualAgentStatus::Piped { pipes: vec![pipe] },
            VirtualAgentStatus::Piped { pipes } => VirtualAgentStatus::Piped { pipes: pipes.clone().into_iter().chain([pipe]).collect() },
            VirtualAgentStatus::Replica { .. } => return Err(MegaphoneError::BadRequest(format!("Cannot pipe agent because it is already a replica"))),
        };
        Ok(())
    }
}

pub enum SyncEvent {
    PipeAgentStart { name: String },
    PipeAgentEnd { name: String },
    ChannelCreated { id: String },
    ChannelDisposed { id: String },
    EventReceived { channel: String, event: EventDto },
}