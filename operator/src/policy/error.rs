use std::sync::Arc;
use std::time::Duration;
use kube_runtime::controller::Action;
use crate::model::context::ContextData;
use crate::model::error::Error;
use crate::spec::Megaphone;

/// an error handler that will be called when the reconciler fails with access to both the
/// object that caused the failure and the actual error
pub fn error_policy(_obj: Arc<Megaphone>, _error: &Error, _ctx: Arc<ContextData>) -> Action {
    Action::requeue(Duration::from_secs(60))
}