use std::collections::hash_map::RandomState;
use std::sync::Arc;

use dashmap::DashMap;
use dashmap::mapref::multiple::RefMulti;
use lazy_static::lazy_static;
use rand::seq::IteratorRandom;

use crate::core::config::{AgentConfig, VirtualAgentMode};
use crate::core::error::MegaphoneError;

pub struct AgentsManagerService {
    virtual_agents: Arc<DashMap<String, VirtualAgentMode>>,
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
            .map(|(k, v)| Self::validate_agent_name(&k).map(|_| (k, v)))
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

    pub fn random_master_id(&self) -> String {
        self.virtual_agents.iter()
            .filter(|entry| matches!(entry.value(), VirtualAgentMode::Master))
            .map(|entry| entry.key().to_string())
            .choose(&mut rand::thread_rng())
            .expect("Cannot select random agent-id")
    }

    pub fn list_agents(&self) -> impl Iterator<Item=RefMulti<String, VirtualAgentMode, RandomState>> {
        self.virtual_agents.iter()
    }

    pub fn add_master(&self, name: &str) -> Result<(), MegaphoneError> {
        Self::validate_agent_name(name)?;
        self.virtual_agents.insert(String::from(name), VirtualAgentMode::Master);
        Ok(())
    }
}