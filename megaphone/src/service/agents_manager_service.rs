use std::collections::hash_map::RandomState;
use std::ops::Add;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use dashmap::DashMap;
use dashmap::mapref::multiple::RefMulti;
use lazy_static::lazy_static;
use rand::seq::IteratorRandom;

use crate::core::config::{AgentConfig, VirtualAgentMode};
use crate::core::error::MegaphoneError;

pub const WARMUP_SECS: u64 = 60;

#[derive(Debug)]
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

    pub fn status(&self) -> VirtualAgentStatus {
        self.status
    }

    pub fn change_ts(&self) -> SystemTime {
        self.change_ts
    }

    pub fn is_warming_up(&self) -> bool {
        match self.status {
            VirtualAgentStatus::Master => self.change_ts.add(Duration::from_secs(WARMUP_SECS)).gt(&SystemTime::now()),
            VirtualAgentStatus::Replica { .. } => false,
            VirtualAgentStatus::Piped => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum VirtualAgentStatus {
    Master,
    Replica { pipe_sessions_count: usize },
    Piped,
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

    pub fn list_agents(&self) -> impl Iterator<Item=RefMulti<String, VirtualAgentProps, RandomState>> {
        self.virtual_agents.iter()
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
}