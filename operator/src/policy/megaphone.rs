use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{Container, ContainerPort, EnvVar, EnvVarSource, ObjectFieldSelector, PodSpec, PodTemplateSpec, ResourceRequirements};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::api::{DeleteParams, ObjectMeta, PostParams};
use kube::{Api, Client, Error};
use std::collections::BTreeMap;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

/// Creates a new deployment of `n` pods with the `inanimate/echo-server:latest` docker image inside,
/// where `n` is the number of `replicas` given.
///
/// # Arguments
/// - `client` - A Kubernetes client to create the deployment with.
/// - `name` - Name of the deployment to be created
/// - `replicas` - Number of pod replicas for the Deployment to contain
/// - `namespace` - Namespace to create the Kubernetes Deployment in.
///
/// Note: It is assumed the resource does not already exists for simplicity. Returns an `Error` if it does.
pub async fn deploy(
    client: Client,
    name: &str,
    replicas: i32,
    namespace: &str,
    image: &str,
) -> Result<Deployment, Error> {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());

    // Definition of the deployment. Alternatively, a YAML representation could be used as well.
    let deployment: Deployment = Deployment {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            labels: Some(labels.clone()),
            ..ObjectMeta::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(replicas),
            selector: LabelSelector {
                match_expressions: None,
                match_labels: Some(labels.clone()),
            },
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: name.to_owned(),
                        image: Some(String::from(image)),
                        ports: Some(vec![ContainerPort {
                            container_port: 3000,
                            ..ContainerPort::default()
                        }]),
                        resources: Some(ResourceRequirements {
                            limits: Some([
                                (String::from("cpu"), Quantity(String::from("20m"))),
                                (String::from("memory"), Quantity(String::from("50Mi"))),
                            ].into_iter().collect()),
                            requests: Some([
                                (String::from("cpu"), Quantity(String::from("20m"))),
                                (String::from("memory"), Quantity(String::from("50Mi"))),
                            ].into_iter().collect())
                        }),
                        env: Some(vec![
                            EnvVar {
                                name: String::from("megaphone_agent_name"),
                                value: None,
                                value_from: Some(EnvVarSource {
                                    field_ref: Some(ObjectFieldSelector {
                                        api_version: None,
                                        field_path: String::from("metadata.name"),
                                    }),
                                    ..Default::default()
                                })
                            }
                        ]),
                        ..Container::default()
                    }],
                    ..PodSpec::default()
                }),
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..ObjectMeta::default()
                }),
            },
            ..DeploymentSpec::default()
        }),
        ..Deployment::default()
    };

    // Create the deployment defined above
    let deployment_api: Api<Deployment> = Api::namespaced(client, namespace);
    deployment_api
        .create(&PostParams::default(), &deployment)
        .await
}

/// Deletes an existing deployment.
///
/// # Arguments:
/// - `client` - A Kubernetes client to delete the Deployment with
/// - `name` - Name of the deployment to delete
/// - `namespace` - Namespace the existing deployment resides in
///
/// Note: It is assumed the deployment exists for simplicity. Otherwise returns an Error.
pub async fn delete(client: Client, name: &str, namespace: &str) -> Result<(), Error> {
    let api: Api<Deployment> = Api::namespaced(client, namespace);
    api.delete(name, &DeleteParams::default()).await?;
    Ok(())
}