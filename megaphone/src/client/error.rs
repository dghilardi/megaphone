use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid url '{url}'")]
    InvalidUrl { url: String },
}