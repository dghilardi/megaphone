use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{
    ConfigMap, ConfigMapVolumeSource, Container, ContainerPort, PodSpec, PodTemplateSpec, Service,
    ServicePort, ServiceSpec, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};

pub fn nginx_configmap() -> ConfigMap {
    ConfigMap {
        metadata: ObjectMeta {
            name: Some(String::from("nginx-configmap")),
            ..Default::default()
        },
        data: Some([
            (String::from("default.conf"), String::from(r#"
                server {
                    listen 80;
                    location / {
                        root /bin/www/;

                        index index.html index.htm;
                        try_files $uri $uri/ /index.html;

                    }
                    location ~ /create {
                        resolver kube-dns.kube-system.svc.cluster.local;
                        proxy_pass http://megaphone-service.default.svc.cluster.local:3000;
                    }
                    location ~ ^/write/([A-Za-z0-9_\-]+)\.([A-Za-z0-9_\-]+)/([A-Za-z0-9_\-]+)$ {
                        resolver kube-dns.kube-system.svc.cluster.local;
                        proxy_pass http://$1.megaphone-headless.default.svc.cluster.local:3000/write/$1.$2/$3;
                    }
                    location ~ ^/read/([A-Za-z0-9_\-]+)\.([A-Za-z0-9_\-.]+)$ {
                        resolver kube-dns.kube-system.svc.cluster.local;

                        proxy_pass http://$1.megaphone-headless.default.svc.cluster.local:3000/read/$1.$2;

                        proxy_http_version 1.1;
                        proxy_set_header Upgrade $http_upgrade;

                        proxy_set_header Host $host;
                        proxy_cache_bypass $http_upgrade;
                    }
                }
            "#))
        ].into_iter().collect()),
        ..Default::default()
    }
}
pub fn nginx_deployment() -> Deployment {
    Deployment {
        metadata: ObjectMeta {
            name: Some(String::from("nginx")),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            selector: LabelSelector {
                match_labels: Some(
                    [(String::from("app"), String::from("nginx"))]
                        .into_iter()
                        .collect(),
                ),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(
                        [(String::from("app"), String::from("nginx"))]
                            .into_iter()
                            .collect(),
                    ),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: String::from("nginx"),
                        image: Some(String::from("nginx:1.25.5-alpine")),
                        ports: Some(vec![ContainerPort {
                            container_port: 80,
                            ..Default::default()
                        }]),
                        volume_mounts: Some(vec![VolumeMount {
                            name: String::from("nginx-config"),
                            mount_path: String::from("/etc/nginx/conf.d"),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    }],
                    volumes: Some(vec![Volume {
                        name: String::from("nginx-config"),
                        config_map: Some(ConfigMapVolumeSource {
                            name: Some(String::from("nginx-configmap")),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn nginx_svc() -> Service {
    Service {
        metadata: ObjectMeta {
            name: Some(String::from("nginx-service")),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            ports: Some(vec![ServicePort {
                name: Some(String::from("http")),
                port: 80,
                ..Default::default()
            }]),
            selector: Some(
                [(String::from("app"), String::from("nginx"))]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    }
}
