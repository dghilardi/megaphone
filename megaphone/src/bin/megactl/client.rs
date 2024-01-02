use std::error::Error;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use axum::body::Body;
use http_body_util::BodyExt;
use hyper::{http, Method, Request, Response, Uri};
use hyper::body::{Bytes, Incoming};
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::net::UnixStream;

use megaphone::dto::error::ErrorDto;

pub struct SimpleRest<C> {
    client: C,
}

impl <C: HyperClient> From<C> for SimpleRest<C> {
    fn from(client: C) -> Self {
        Self {
            client,
        }
    }
}

impl <C> SimpleRest<C>
    where
        C: HyperClient,
{
    pub async fn get<U, Res>(&self, url: U) -> anyhow::Result<Res>
        where
            U: TryInto<Uri>,
            U::Error: Error + Send + Sync + 'static,
            Res: DeserializeOwned,
    {
        let http_req = Request::builder()
            .method(Method::GET)
            .uri(url.try_into()?)
            .body(Body::empty())
            .context("request builder")?;

        let res = self.client.send(http_req).await?;
        if !res.status().is_success() {
            bail!("Service invocation error - {}", Self::extract_error_message(res).await)
        }
        let res_body = res.collect().await?.to_bytes();
        let parsed_res = serde_json::from_slice(&res_body[..])?;
        Ok(parsed_res)
    }

    pub async fn delete<U, Res>(&self, url: U) -> anyhow::Result<Res>
        where
            U: TryInto<Uri>,
            U::Error: Error + Send + Sync + 'static,
            Res: DeserializeOwned,
    {
        let http_req = Request::builder()
            .method(Method::DELETE)
            .uri(url.try_into()?)
            .body(Body::empty())
            .context("request builder")?;

        let res = self.client.send(http_req).await?;
        if !res.status().is_success() {
            bail!("Service invocation error - {}", Self::extract_error_message(res).await)
        }
        let res_body = res.collect().await?.to_bytes();
        let parsed_res = serde_json::from_slice(&res_body[..])?;
        Ok(parsed_res)
    }

    pub async fn post<U, Req, Res>(&self, url: U, req: Req) -> anyhow::Result<Res>
        where
            U: TryInto<Uri>,
            U::Error: Error + Send + Sync + 'static,
            Req: Serialize,
            Res: DeserializeOwned,
    {
        let http_req = Request::builder()
            .method(Method::POST)
            .uri(url.try_into()?)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&req)?))
            .context("request builder")?;

        let res = self.client.send(http_req).await?;
        if !res.status().is_success() {
            bail!("Service invocation error - {}", Self::extract_error_message(res).await)
        }
        let res_body = res.collect().await?.to_bytes();
        let parsed_res = serde_json::from_slice(&res_body[..])?;
        Ok(parsed_res)
    }

    async fn extract_error_message(res: Response<Incoming>) -> String {
        match res.collect().await.and_then(|bytes| Ok(serde_json::from_slice::<ErrorDto>(bytes.to_bytes().as_ref()))) {
            Ok(Ok(err_dto)) => format!("Megaphone error - [{}] {}", err_dto.code, err_dto.message),
            Ok(Err(err)) => format!("Deserialization error - {err}"),
            Err(err) => format!("Error extracting response body - {err}"),
        }
    }
}

pub trait HyperClient {
    async fn send<B>(&self, req: Request<B>) -> anyhow::Result<Response<Incoming>>
        where
            B: hyper::body::Body + 'static,
            B::Data: Send,
            B::Error: Error + Send + Sync,
    ;
}

pub struct UnixClient {
}

impl UnixClient {
    pub fn new() -> Self { Self {} }
}

impl HyperClient for UnixClient {
    async fn send<B>(&self, req: Request<B>) -> anyhow::Result<Response<Incoming>>
    where
        B: hyper::body::Body + 'static,
        B::Data: Send,
        B::Error: Error + Send + Sync,
    {
        let socket_path = String::from_utf8(hex::decode(req.uri().host().unwrap_or_default())?)?;
        log::debug!("unix socket: {socket_path} => [{}] {}", req.method(), req.uri().path());
        let stream = TokioIo::new(UnixStream::connect(&socket_path).await?);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await?;
        sender.send_request(req).await.map_err(anyhow::Error::from)
    }
}

pub struct TcpSocketUri {
    socket_addr: PathBuf,
    path: String,
}

impl TcpSocketUri {
    pub fn new(socket: impl AsRef<Path>, path: impl AsRef<str>) -> Self {
        Self {
            socket_addr: socket.as_ref().to_path_buf(),
            path: path.as_ref().to_string(),
        }
    }
}

impl TryFrom<TcpSocketUri> for Uri {

    type Error = http::Error;

    fn try_from(value: TcpSocketUri) -> Result<Self, Self::Error> {
        Uri::builder()
            .scheme("unix")
            .authority(hex::encode(value.socket_addr.to_str().unwrap_or_default().as_bytes()))
            .path_and_query(value.path)
            .build()
    }
}