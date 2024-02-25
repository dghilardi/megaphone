use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ErrorDto {
    pub code: String,
    pub message: String,
}

