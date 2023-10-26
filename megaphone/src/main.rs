use std::fs;
use std::future::ready;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Context;

use axum::{Router, routing::{get, post}, Server};
use axum::extract::FromRef;
use axum::handler::Handler;
use axum::response::IntoResponse;
use axum::routing::IntoMakeService;
use futures::StreamExt;
use hyperlocal::{SocketIncoming, UnixServerExt};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use tokio::sync::RwLock;
use tokio::try_join;

use crate::core::config::{compose_config, MegaphoneConfig};
use crate::dto::message::EventDto;
use crate::service::agents_manager_service::AgentsManagerService;
use crate::service::megaphone_service::{CHANNEL_DURATION_METRIC_NAME, MegaphoneService};
use crate::state::MegaphoneState;

pub mod service;
mod dto;
mod core;
mod http;
mod state;

fn spawn_buffer_cleaner(svc: MegaphoneService<EventDto>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            svc.drop_expired();
        }
    });
}

fn setup_metrics_recorder() -> PrometheusHandle {
    const EXPONENTIAL_SECONDS: &[f64] = &[
        80.0, 160.0, 320.0, 640.0, 1280.0, 2560.0, 5120.0, 10240.0, 20480.0,
    ];

    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(String::from(CHANNEL_DURATION_METRIC_NAME)),
            EXPONENTIAL_SECONDS,
        )
        .unwrap()
        .install_recorder()
        .unwrap()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();
    let app_config: MegaphoneConfig = compose_config("megaphone", "megaphone")
        .expect("Error loading configuration");

    let address = app_config.address.clone();
    let mng_socket_path = app_config.mng_socket_path.clone();
    let service = MegaphoneState::build(app_config)
        .expect("Error building megaphone state");

    spawn_buffer_cleaner(FromRef::from_ref(&service));

    let recorder_handle = setup_metrics_recorder();

    let app = Router::new()

        .route("/create", post(http::channel::create_handler))
        .route("/write/:channel_id/:stream_id", post(http::channel::write_handler))
        .route("/read/:id", get(http::channel::read_handler))
        .route("/channelsExists", post(http::channel::channel_exists_handler))
        .route("/metrics", get(move || ready(recorder_handle.render())))
        .with_state(service.clone());

    try_join!(
        axum::Server::bind(&address)
            .serve(app.into_make_service()),
        build_server(mng_socket_path, service)
            .expect("Error building mgmt server")
    ).expect("Error starting server");
}

pub fn build_server(path: impl AsRef<Path>, service: MegaphoneState<EventDto>) -> anyhow::Result<Server<SocketIncoming, IntoMakeService<Router>>> {
    if path.as_ref().exists() {
        fs::remove_file(path.as_ref())
            .context("Could not remove old socket!")?;
    }

    let app = Router::new()
        .route("/vagent/list", get(http::vagent::list_virtual_agents))
        .route("/vagent/add", post(http::vagent::add_virtual_agent))
        .with_state(service);

    let srv = axum::Server::bind_unix(path)?
        .serve(app.into_make_service());

    Ok(srv)
}