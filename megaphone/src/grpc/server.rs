use tonic::{transport::Server, Request, Response, Status};

pub mod megaphone {
    tonic::include_proto!("megaphone"); // The string specified here must match the proto package name
}