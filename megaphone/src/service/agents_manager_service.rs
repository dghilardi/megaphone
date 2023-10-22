use std::collections::hash_map::RandomState;
use std::sync::Arc;

use dashmap::DashMap;
use dashmap::mapref::multiple::RefMulti;
use rand::seq::IteratorRandom;

use crate::core::config::{AgentConfig, VirtualAgentMode};

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
    pub fn new(conf: AgentConfig) -> Self {
        Self {
            virtual_agents: Arc::new(conf.virtual_agents
                .into_iter()
                .collect()
            ),
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
}