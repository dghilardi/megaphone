use std::collections::hash_map::RandomState;
use std::sync::Arc;
use std::time::SystemTime;

use dashmap::DashMap;
use dashmap::mapref::multiple::RefMulti;
use lazy_static::lazy_static;
use rand::seq::IteratorRandom;

use crate::core::config::{AgentConfig, VirtualAgentMode};
use crate::core::error::MegaphoneError;

pub struct VirtualAgentProps {
    change_ts: SystemTime,
    status: VirtualAgentStatus,
}

impl VirtualAgentProps {
    pub fn new(mode: VirtualAgentMode) -> Self {
        Self {
            change_ts: SystemTime::now(),
            status: match mode {
                VirtualAgentMode::Master => VirtualAgentStatus::Master,
                VirtualAgentMode::Replica => VirtualAgentStatus::Replica,
            }
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
}

#[derive(Clone, Copy)]
pub enum VirtualAgentStatus {
    Master,
    Replica,
    Piped,
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
            .map(|(k, v)| Self::validate_agent_name(&k).map(|_| (k, VirtualAgentProps::new(v))))
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

    pub fn random_master_id(&self) -> Result<String, MegaphoneError> {
        self.virtual_agents.iter()
            .filter(|entry| matches!(entry.value().status(), VirtualAgentStatus::Master))
            .map(|entry| entry.key().to_string())
            .choose(&mut rand::thread_rng())
            .ok_or(MegaphoneError::InternalError(String::from("No virtual agent with master status was found")))
    }

    pub fn list_agents(&self) -> impl Iterator<Item=RefMulti<String, VirtualAgentProps, RandomState>> {
        self.virtual_agents.iter()
    }

    pub fn add_master(&self, name: &str) -> Result<(), MegaphoneError> {
        Self::validate_agent_name(name)?;
        self.virtual_agents.insert(String::from(name), VirtualAgentProps::new(VirtualAgentMode::Master));
        Ok(())
    }
}