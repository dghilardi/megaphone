use std::sync::Arc;
use std::time::Duration;
use kube::{Client, Resource, ResourceExt};
use kube_runtime::controller::Action;
use super::finalizer;
use crate::model::context::ContextData;
use crate::model::error::Error;
use crate::policy::megaphone;
use crate::spec::Megaphone;

/// Action to be taken upon an `Echo` resource during reconciliation
enum MegaphoneAction {
    /// Create the subresources, this includes spawning `n` pods with Echo service
    Create,
    /// Delete all subresources created in the `Create` phase
    Delete,
    /// This `Echo` resource is in desired state and requires no actions to be taken
    NoOp,
}

/// The reconciler that will be called when either object change
pub async fn reconcile(megaphone: Arc<Megaphone>, context: Arc<ContextData>) -> Result<Action, Error> {
    let client: Client = context.client.clone();


    let namespace: String = match megaphone.namespace() {
        None => {
            // If there is no namespace to deploy to defined, reconciliation ends with an error immediately.
            return Err(Error::UserInputError(
                "Expected Echo resource to be namespaced. Can't deploy to an unknown namespace."
                    .to_owned(),
            ));
        }
        Some(namespace) => namespace,
    };
    let name = megaphone.name_any(); // Name of the Echo resource is used to name the subresources as well.

    // Performs action as decided by the `determine_action` function.
    return match determine_action(&megaphone) {
        MegaphoneAction::Create => {
            // Creates a deployment with `n` Echo service pods, but applies a finalizer first.
            // Finalizer is applied first, as the operator might be shut down and restarted
            // at any time, leaving subresources in intermediate state. This prevents leaks on
            // the `Echo` resource deletion.

            // Apply the finalizer first. If that fails, the `?` operator invokes automatic conversion
            // of `kube::Error` to the `Error` defined in this crate.
            finalizer::add(client.clone(), &name, &namespace).await?;
            // Invoke creation of a Kubernetes built-in resource named deployment with `n` echo service pods.
            megaphone::deploy(client, &name, megaphone.spec.replicas, &namespace).await?;
            Ok(Action::requeue(Duration::from_secs(10)))
        }
        MegaphoneAction::Delete => {
            // Deletes any subresources related to this `Echo` resources. If and only if all subresources
            // are deleted, the finalizer is removed and Kubernetes is free to remove the `Echo` resource.

            //First, delete the deployment. If there is any error deleting the deployment, it is
            // automatically converted into `Error` defined in this crate and the reconciliation is ended
            // with that error.
            // Note: A more advanced implementation would check for the Deployment's existence.
            megaphone::delete(client.clone(), &name, &namespace).await?;

            // Once the deployment is successfully removed, remove the finalizer to make it possible
            // for Kubernetes to delete the `Echo` resource.
            finalizer::delete(client, &name, &namespace).await?;
            Ok(Action::await_change()) // Makes no sense to delete after a successful delete, as the resource is gone
        }
        // The resource is already in desired state, do nothing and re-check after 10 seconds
        MegaphoneAction::NoOp => Ok(Action::requeue(Duration::from_secs(10))),
    };
}

fn determine_action(megaphone: &Megaphone) -> MegaphoneAction {
    return if megaphone.meta().deletion_timestamp.is_some() {
        MegaphoneAction::Delete
    } else if megaphone
        .meta()
        .finalizers
        .as_ref()
        .map_or(true, |finalizers| finalizers.is_empty())
    {
        MegaphoneAction::Create
    } else {
        MegaphoneAction::NoOp
    };
}