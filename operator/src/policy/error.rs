use std::sync::Arc;
use std::time::Duration;
use kube::runtime::controller::Action;
use crate::model::context::ContextData;
use crate::model::error::Error;
use crate::model::spec::Megaphone;

/// an error handler that will be called when the reconciler fails with access to both the
/// object that caused the failure and the actual error
pub fn error_policy(_obj: Arc<Megaphone>, error: &Error, _ctx: Arc<ContextData>) -> Action {
    println!("reconcile failed internal error: {:?}", error);
    Action::requeue(Duration::from_secs(60))
}