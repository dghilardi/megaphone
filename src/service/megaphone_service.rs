use std::io;
use axum::BoxError;
use axum::response::ErrorResponse;
use uuid::Uuid;

pub struct MegaphoneService {

}

impl MegaphoneService {
    pub fn new() -> Self {
        Self { }
    }

    pub async fn create_channel(&self) -> String {
        Uuid::new_v4().to_string()
    }

    pub async fn read_channel(&self) -> impl futures::stream::Stream<Item=Result<String, BoxError>> {
        futures::stream::iter([
            Ok(String::from("Hello")),
            Ok(String::from("world")),
        ])
    }
}