use k8s_openapi::api::networking::v1::{Ingress, IngressBackend, IngressServiceBackend, IngressSpec, ServiceBackendPort};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

pub fn ingress() -> Ingress {
    Ingress {
        metadata: ObjectMeta {
            name: Some(String::from("ingress")),
            ..Default::default()
        },
        spec: Some(IngressSpec {
            default_backend: Some(IngressBackend {
                service: Some(IngressServiceBackend {
                    name: String::from("nginx-service"),
                    port: Some(ServiceBackendPort {
                        number: Some(80),
                        ..Default::default()
                    })
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}