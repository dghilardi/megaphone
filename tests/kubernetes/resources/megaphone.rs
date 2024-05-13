use crate::docker::image::MEGAPHONE_IMAGE_NAME;
use k8s_openapi::api;
use k8s_openapi::api::apps::v1::{StatefulSet, StatefulSetSpec};
use k8s_openapi::api::core::v1::{
    ContainerPort, EnvVar, EnvVarSource, ObjectFieldSelector, PodSpec, PodTemplateSpec,
    ResourceRequirements, Service, ServicePort, ServiceSpec,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};

pub fn megaphone_sts(replicas: i32) -> StatefulSet {
    StatefulSet {
        metadata: ObjectMeta {
            name: Some(String::from("megaphone")),
            ..Default::default()
        },
        spec: Some(StatefulSetSpec {
            replicas: Some(replicas),
            selector: LabelSelector {
                match_labels: Some(
                    [(String::from("app"), String::from("megaphone"))]
                        .into_iter()
                        .collect(),
                ),
                ..Default::default()
            },
            service_name: String::from("megaphone-headless"),
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(
                        [
                            (String::from("app"), String::from("megaphone")),
                            (String::from("acceptNewChannels"), String::from("yes")),
                        ]
                        .into_iter()
                        .collect(),
                    ),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![api::core::v1::Container {
                        name: String::from("megaphone"),
                        image: Some(String::from(MEGAPHONE_IMAGE_NAME)),
                        ports: Some(vec![ContainerPort {
                            container_port: 3000,
                            ..Default::default()
                        }]),
                        image_pull_policy: Some(String::from("Never")),
                        resources: Some(ResourceRequirements {
                            limits: Some(
                                [
                                    (String::from("cpu"), Quantity(String::from("20m"))),
                                    (String::from("memory"), Quantity(String::from("50Mi"))),
                                ]
                                .into_iter()
                                .collect(),
                            ),
                            ..Default::default()
                        }),
                        env: Some(vec![
                            EnvVar {
                                name: String::from("megaphone_agent"),
                                value: None,
                                value_from: Some(EnvVarSource {
                                    field_ref: Some(ObjectFieldSelector {
                                        api_version: None,
                                        field_path: String::from("metadata.name"),
                                    }),
                                    ..Default::default()
                                }),
                            },
                            EnvVar {
                                name: String::from("megaphone_agent_warmup_secs"),
                                value: Some(String::from("0")),
                                ..Default::default()
                            },
                        ]),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        status: None,
    }
}

pub fn megaphone_svc() -> Service {
    Service {
        metadata: ObjectMeta {
            name: Some(String::from("megaphone-service")),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            ports: Some(vec![ServicePort {
                name: Some(String::from("api")),
                port: 3000,
                ..Default::default()
            }]),
            selector: Some(
                [
                    (String::from("app"), String::from("megaphone")),
                    (String::from("acceptNewChannels"), String::from("yes")),
                ]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn megaphone_headless_svc() -> Service {
    Service {
        metadata: ObjectMeta {
            name: Some(String::from("megaphone-headless")),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            cluster_ip: Some(String::from("None")),
            ports: Some(vec![ServicePort {
                name: Some(String::from("api")),
                port: 3000,
                ..Default::default()
            }]),
            selector: Some(
                [(String::from("app"), String::from("megaphone"))]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    }
}
