use std::sync::Arc;

use axum::extract::FromRef;
use tokio::sync::RwLock;

use crate::core::config::MegaphoneConfig;
use crate::core::error::MegaphoneError;
use crate::service::agents_manager_service::AgentsManagerService;
use crate::service::megaphone_service::MegaphoneService;

pub struct MegaphoneState<Evt> {
    megaphone_cfg: Arc<RwLock<MegaphoneConfig>>,
    megaphone_svc: MegaphoneService<Evt>,
    agents_manager_svc: AgentsManagerService,
}

impl <Evt> MegaphoneState<Evt> {
    pub fn build(app_config: MegaphoneConfig) -> Result<Self, MegaphoneError> {
        let agents_manager = AgentsManagerService::new(app_config.agent.clone())?;

        Ok(MegaphoneState {
            megaphone_cfg: Arc::new(RwLock::new(app_config)),
            megaphone_svc: MegaphoneService::new(agents_manager.clone()),
            agents_manager_svc: agents_manager,
        })
    }
}

impl <Evt> Clone for MegaphoneState<Evt> {
    fn clone(&self) -> Self {
        Self {
            agents_manager_svc: self.agents_manager_svc.clone(),
            megaphone_cfg: self.megaphone_cfg.clone(),
            megaphone_svc: self.megaphone_svc.clone(),
        }
    }
}

impl <Evt> FromRef<MegaphoneState<Evt>> for MegaphoneService<Evt> {
    fn from_ref(app_state: &MegaphoneState<Evt>) -> Self {
        app_state.megaphone_svc.clone()
    }
}

impl <Evt> FromRef<MegaphoneState<Evt>> for Arc<RwLock<MegaphoneConfig>> {
    fn from_ref(app_state: &MegaphoneState<Evt>) -> Self {
        app_state.megaphone_cfg.clone()
    }
}

impl <Evt> FromRef<MegaphoneState<Evt>> for AgentsManagerService {
    fn from_ref(app_state: &MegaphoneState<Evt>) -> Self {
        app_state.agents_manager_svc.clone()
    }
}