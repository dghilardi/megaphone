use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid url '{url}'")]
    InvalidUrl { url: String },
}

#[derive(Error, Debug)]
pub enum DelayedResponseError {
    #[error("Initialization error - {0}")]
    InitializationError(String),
    #[error("Missing response")]
    MissingResponse,
    #[error("Deserialization error - {0}")]
    DeserializationError(String),
}