use std::ops::Add;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use dashmap::DashMap;
use dashmap::mapref::multiple::RefMulti;
use lazy_static::lazy_static;
use megaphone::dto::agent::VirtualAgentModeDto;
use megaphone::dto::message::EventDto;
use rand::random;
use rand::seq::IteratorRandom;
use ring::aead::{Aad, AES_256_GCM, LessSafeKey, Nonce, UnboundKey};
use tokio::sync::mpsc;

use crate::core::config::{AgentConfig, VirtualAgentMode};
use crate::core::error::MegaphoneError;
use crate::service::megaphone_service::ChannelShortId;

pub const WARMUP_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct VirtualAgentProps {
    key: [u8; 32],
    change_ts: SystemTime,
    status: VirtualAgentStatus,
}

impl VirtualAgentProps {
    pub fn new(mode: VirtualAgentStatus) -> Self {
        Self {
            key: random(),
            change_ts: SystemTime::now(),
            status: mode,
        }
    }

    pub fn new_with_key(mode: VirtualAgentStatus, key: [u8;32]) -> Self {
        Self {
            key,
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

    pub fn status_mut(&mut self) -> &mut VirtualAgentStatus {
        &mut self.status
    }

    pub fn change_ts(&self) -> SystemTime {
        self.change_ts
    }

    pub fn is_warming_up(&self) -> bool {
        match self.status() {
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

impl From<&VirtualAgentStatus> for VirtualAgentModeDto {
    fn from(value: &VirtualAgentStatus) -> Self {
        match value {
            VirtualAgentStatus::Master => Self::Master,
            VirtualAgentStatus::Replica { .. } => Self::Replica,
            VirtualAgentStatus::Piped { .. } => Self::Piped,
        }
    }
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

    pub fn open_replica_session(&self, name: &str, key: [u8;32]) -> Result<(), MegaphoneError> {
        let mut entry = self.virtual_agents.entry(String::from(name))
            .or_insert_with(|| VirtualAgentProps::new_with_key(VirtualAgentStatus::Replica { pipe_sessions_count: 0 }, key));

        let VirtualAgentStatus::Replica { ref mut pipe_sessions_count } = entry.status_mut() else {
            return Err(MegaphoneError::InternalError(format!("{name} agent is already registered but is not a replica")));
        };
        *pipe_sessions_count += 1;

        Ok(())
    }

    pub fn close_replica_session(&self, name: &str) -> Result<(), MegaphoneError> {
        let Some(mut entry) = self.virtual_agents.get_mut(name) else {
            return Err(MegaphoneError::InternalError(format!("{name} agent is not registered")));
        };
        let VirtualAgentStatus::Replica { ref mut pipe_sessions_count } = entry.status_mut() else {
            return Err(MegaphoneError::InternalError(format!("{name} agent is already registered but is not a replica")));
        };
        *pipe_sessions_count -= 1;
        Ok(())
    }

    pub fn is_agent_distributed(&self, name: &str) -> Result<bool, MegaphoneError> {
        let Some(agent) = self.virtual_agents.get(name) else {
            return Err(MegaphoneError::InternalError(format!("Agent {name} is not registered")));
        };
        match agent.status() {
            VirtualAgentStatus::Master => Ok(false),
            VirtualAgentStatus::Replica { pipe_sessions_count: 0 } => Ok(false),
            VirtualAgentStatus::Replica { .. } => Ok(true),
            VirtualAgentStatus::Piped { .. } => Ok(true),
        }
    }

    pub fn get_pipes(&self, name: &str) -> Vec<mpsc::Sender<SyncEvent>> {
        self.virtual_agents.get(name)
            .and_then(|agent| if let VirtualAgentStatus::Piped { pipes } = &agent.status() { Some(pipes.clone()) } else { None })
            .unwrap_or_default()
    }

    pub fn register_pipe(&self, name: &str, pipe: mpsc::Sender<SyncEvent>) -> Result<(), MegaphoneError> {
        let Some(mut agent) = self.virtual_agents.get_mut(name) else {
            return Err(MegaphoneError::BadRequest(format!("Agent {name} is not registered")))
        };
        let Ok(()) = pipe.try_send(SyncEvent::PipeAgentStart { name: name.to_string(), key: agent.key }) else {
            return Err(MegaphoneError::InternalError(format!("Error sending pipe registration event for agent {name}")))
        };

        let new_status = match agent.status() {
            VirtualAgentStatus::Master => VirtualAgentStatus::Piped { pipes: vec![pipe] },
            VirtualAgentStatus::Piped { pipes } => VirtualAgentStatus::Piped { pipes: pipes.clone().into_iter().chain([pipe]).collect() },
            VirtualAgentStatus::Replica { pipe_sessions_count: 0 } => VirtualAgentStatus::Piped { pipes: vec![pipe] },
            VirtualAgentStatus::Replica { .. } => return Err(MegaphoneError::BadRequest(String::from("Cannot pipe agent because it is already a replica"))),
        };
        agent.change_status(new_status);
        Ok(())
    }

    pub fn encrypt_channel_id(&self, agent_id: &str, id: ChannelShortId) -> Result<String, MegaphoneError> {
        let agent = self.virtual_agents.get(agent_id)
            .ok_or(MegaphoneError::InternalError(format!("Agent {agent_id} is not registered")))?;

        // Create a new AEAD key without a designated role or nonce sequence
        let unbound_key = UnboundKey::new(&AES_256_GCM, &agent.key)
            .map_err(|err| MegaphoneError::InternalError(format!("Cannot build the key - {err}")))?;

        // Create a new NonceSequence type which generates nonces
        let nonce: [u8; 12] = random();
        let nonce_sequence = Nonce::assume_unique_for_key(nonce);

        // Create a new AEAD key for encrypting and signing ("sealing"), bound to a nonce sequence
        // The SealingKey can be used multiple times, each time a new nonce will be used
        let sealing_key = LessSafeKey::new(unbound_key);

        // Create a mutable copy of the data that will be encrypted in place
        let mut data = id.0
            .to_be_bytes()
            .to_vec();

        sealing_key.seal_in_place_append_tag(nonce_sequence,Aad::empty(), &mut data)
            .map_err(|err| MegaphoneError::InternalError(format!("Cannot cipher key - {err}")))?;

        log::debug!("nonce {:X?} data {:X?}", nonce, data);

        let full_data = nonce.into_iter()
            .chain(data)
            .collect::<Vec<_>>();

        Ok(URL_SAFE_NO_PAD.encode(full_data))
    }

    pub fn decrypt_channel_id(&self, agent_id: &str, input: &str) -> Result<ChannelShortId, MegaphoneError> {
        let agent = self.virtual_agents.get(agent_id)
            .ok_or(MegaphoneError::InternalError(format!("Agent {agent_id} is not registered")))?;

        let data = URL_SAFE_NO_PAD.decode(input.as_bytes())
            .map_err(|err| MegaphoneError::BadRequest(format!("Cannot deserialize {input} - {err}")))?;

        log::debug!("nonce {:X?} data {:X?}", &data[..12], &data[12..]);

        let nonce = data[..12].try_into()
            .map_err(|v| MegaphoneError::InternalError(format!("Wrong IV size. Expected 12 - {v}")))?;

        let unbound_key = UnboundKey::new(&AES_256_GCM, &agent.key)
            .map_err(|err| MegaphoneError::InternalError(format!("Cannot create key - {err}")))?;

        let nonce_sequence = Nonce::assume_unique_for_key(nonce);

        let opening_key = LessSafeKey::new(unbound_key);

        let mut data = data[12..].to_vec();

        let decrypted = opening_key.open_in_place(nonce_sequence, Aad::empty(), &mut data)
            .map_err(|err| MegaphoneError::BadRequest(format!("Cannot deserialize data - {err}")))?;

        Ok(ChannelShortId(u128::from_be_bytes(decrypted.try_into().unwrap())))
    }
}

pub enum SyncEvent {
    PipeAgentStart { name: String, key: [u8;32] },
    PipeAgentEnd { name: String },
    ChannelCreated { id: String },
    ChannelDisposed { id: String },
    EventReceived { channel: String, event: EventDto },
}