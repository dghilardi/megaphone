use hyper::{Body, Method, Request, Uri};
use hyper::body::{Bytes, HttpBody};
use hyper::client::connect::Connect;
use std::error::Error as StdError;
use anyhow::{bail, Context};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub struct SimpleRest<C, B = Body> {
    client: hyper::Client<C, B>
}

impl <C, B> From<hyper::Client<C, B>> for SimpleRest<C, B> {
    fn from(value: hyper::Client<C, B>) -> Self {
        Self {
            client: value,
        }
    }
}

impl <C, B> SimpleRest<C, B>
    where
        C: Connect + Clone + Send + Sync + 'static,
        B: HttpBody + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub async fn get<U, Res>(&self, url: U) -> anyhow::Result<Res>
        where
            B: Default,
            U: Into<Uri>,
            Res: DeserializeOwned,
    {
        let res = self.client.get(url.into()).await?;
        if !res.status().is_success() {
            bail!("Response error")
        }
        let res_body: Bytes = hyper::body::to_bytes(res.into_body()).await?;
        let parsed_res = serde_json::from_slice(&res_body[..])?;
        Ok(parsed_res)
    }

    pub async fn delete<U, Res>(&self, url: U) -> anyhow::Result<Res>
        where
            B: Default + From<Vec<u8>>,
            U: Into<Uri>,
            Res: DeserializeOwned,
    {
        let http_req = Request::builder()
            .method(Method::DELETE)
            .uri(url.into())
            .body(B::from(Vec::<u8>::new()))
            .context("request builder")?;

        let res = self.client.request(http_req).await?;
        if !res.status().is_success() {
            bail!("Response error")
        }
        let res_body: Bytes = hyper::body::to_bytes(res.into_body()).await?;
        let parsed_res = serde_json::from_slice(&res_body[..])?;
        Ok(parsed_res)
    }

    pub async fn post<U, Req, Res>(&self, url: U, req: Req) -> anyhow::Result<Res>
        where
            B: Default + From<Vec<u8>>,
            U: Into<Uri>,
            Req: Serialize,
            Res: DeserializeOwned,
    {
        let http_req = Request::builder()
            .method(Method::POST)
            .uri(url.into())
            .header("Content-Type", "application/json")
            .body(B::from(serde_json::to_vec(&req)?))
            .context("request builder")?;

        let res = self.client.request(http_req).await?;
        if !res.status().is_success() {
            bail!("Response error")
        }
        let res_body: Bytes = hyper::body::to_bytes(res.into_body()).await?;
        let parsed_res = serde_json::from_slice(&res_body[..])?;
        Ok(parsed_res)
    }
}