use anyhow::{bail, Context};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{Method, Request, Response, Uri};
use hyper_util::client::legacy::connect::Connect;
use hyper_util::client::legacy::Client;
use serde::de::DeserializeOwned;
use serde::Serialize;

use megaphone::dto::error::ErrorDto;

pub struct SimpleRest<C> {
    client: Client<C, Full<Bytes>>,
}

impl<C> From<Client<C, Full<Bytes>>> for SimpleRest<C> {
    fn from(value: Client<C, Full<Bytes>>) -> Self {
        Self { client: value }
    }
}

impl<C> SimpleRest<C>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    pub async fn get<U, Res>(&self, url: U) -> anyhow::Result<Res>
    where
        U: Into<Uri>,
        Res: DeserializeOwned,
    {
        let res = self.client.get(url.into()).await?;
        if !res.status().is_success() {
            bail!(
                "Service invocation error - {}",
                Self::extract_error_message(res).await
            )
        }
        let res_body: Bytes = res.into_body().collect().await?.to_bytes();
        let parsed_res = serde_json::from_slice(&res_body[..])?;
        Ok(parsed_res)
    }

    pub async fn delete<U, Res>(&self, url: U) -> anyhow::Result<Res>
    where
        U: Into<Uri>,
        Res: DeserializeOwned,
    {
        let http_req = Request::builder()
            .method(Method::DELETE)
            .uri(url.into())
            .body(Full::default())
            .context("request builder")?;

        let res = self.client.request(http_req).await?;
        if !res.status().is_success() {
            bail!(
                "Service invocation error - {}",
                Self::extract_error_message(res).await
            )
        }
        let res_body: Bytes = res.into_body().collect().await?.to_bytes();
        let parsed_res = serde_json::from_slice(&res_body[..])?;
        Ok(parsed_res)
    }

    pub async fn post<U, Req, Res>(&self, url: U, req: Req) -> anyhow::Result<Res>
    where
        U: Into<Uri>,
        Req: Serialize,
        Res: DeserializeOwned,
    {
        let http_req = Request::builder()
            .method(Method::POST)
            .uri(url.into())
            .header("Content-Type", "application/json")
            .body(Full::from(serde_json::to_vec(&req)?))
            .context("request builder")?;

        let res = self.client.request(http_req).await?;
        if !res.status().is_success() {
            bail!(
                "Service invocation error - {}",
                Self::extract_error_message(res).await
            )
        }
        let res_body: Bytes = res.into_body().collect().await?.to_bytes();
        let parsed_res = serde_json::from_slice(&res_body[..])?;
        Ok(parsed_res)
    }

    async fn extract_error_message(res: Response<Incoming>) -> String {
        match res
            .into_body()
            .collect()
            .await
            .map(|body| serde_json::from_slice::<ErrorDto>(body.to_bytes().as_ref()))
        {
            Ok(Ok(err_dto)) => format!("Megaphone error - [{}] {}", err_dto.code, err_dto.message),
            Ok(Err(err)) => format!("Deserialization error - {err}"),
            Err(err) => format!("Error extracting response body - {err}"),
        }
    }
}
