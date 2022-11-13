pub mod service;

use std::sync::Arc;
use axum::{routing::{get, post}, Router, Extension};
use axum::body::StreamBody;
use axum::extract::{Path, State};
use axum::handler::Handler;
use axum::http::{header, HeaderMap};
use axum::middleware::AddExtension;
use axum::response::IntoResponse;
use crate::service::megaphone_service::MegaphoneService;

async fn create_handler(
    State(svc): State<Arc<MegaphoneService>>,
) -> String {
    svc.create_channel().await
}

async fn read_handler(
    Path(id): Path<String>,
    State(svc): State<Arc<MegaphoneService>>,
) -> impl IntoResponse {
    let stream = svc.read_channel().await;
    let body = StreamBody::new(stream);

    body
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let service = Arc::new(MegaphoneService::new());

    let app = Router::with_state(service)

        .route("/create", post(create_handler))
        .route("/read/:id", get(read_handler));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .expect("Error starting server");
}