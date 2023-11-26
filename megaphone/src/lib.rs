//! Reverse proxy for long running requests and server streaming.
//!
//! Provides client and server components for asynchronous communication (i.e. from server to client)

pub mod dto;
#[cfg(feature = "client")]
pub mod client;