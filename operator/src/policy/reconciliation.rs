use std::sync::Arc;

use anyhow::Result;
use kube::runtime::controller::Action;

use crate::model::context::ContextData;
use crate::model::error::Error;
use crate::model::spec::Megaphone;
use crate::service::reconciler_svc::MegaphoneReconciler;

pub async fn reconcile(megaphone: Arc<Megaphone>, ctx: Arc<ContextData>) -> Result<Action, Error> {
    let reconciler = MegaphoneReconciler::new(megaphone, ctx)?;
    reconciler.reconcile().await
}