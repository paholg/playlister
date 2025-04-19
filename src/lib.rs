use std::fmt;

use cache::Cache;
use eyre::Context;
use reqwest::{RequestBuilder, StatusCode};
use serde::{Deserialize, de::DeserializeOwned};
use tracing::error;
use track::Track;

pub mod cache;
pub mod reddit;
pub mod spotify;
pub mod tidal;
pub mod track;

#[derive(Deserialize)]
#[serde(transparent)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub fn new(s: T) -> Self {
        Self(s)
    }

    pub fn expose_secret(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<SECRET>")
    }
}

#[derive(Deserialize)]
struct AuthResponse {
    access_token: Secret<String>,
}

#[allow(async_fn_in_trait)]
pub trait Service: Sized {
    type Settings;

    async fn new(data: Data<Self>) -> eyre::Result<Self>;
    async fn run(&self) -> eyre::Result<()>;
}

pub struct Data<S: Service> {
    pub cache: Cache,
    pub client: reqwest::Client,
    pub settings: S::Settings,
    pub tracks: Vec<Track>,
}

impl<S: Service> Data<S> {
    pub fn new(
        cache: &Cache,
        client: &reqwest::Client,
        settings: S::Settings,
        tracks: &[Track],
    ) -> Self {
        Self {
            cache: cache.clone(),
            client: client.clone(),
            settings,
            tracks: tracks.to_owned(),
        }
    }

    pub async fn run(self) {
        let client = match S::new(self).await {
            Ok(client) => client,
            Err(error) => {
                error!(%error, "failed to create client");
                return;
            }
        };

        if let Err(error) = client.run().await {
            error!(%error, "failed to update playlist");
        }
    }
}

#[derive(Debug)]
struct RequestError {
    #[allow(dead_code)]
    msg: String,
    #[allow(dead_code)]
    url: String,
    #[allow(dead_code)]
    status: StatusCode,
    #[allow(dead_code)]
    body: String,
}

impl std::error::Error for RequestError {}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[allow(async_fn_in_trait)]
pub trait JsonRequest {
    async fn send_it(self) -> eyre::Result<()>;
    async fn send_it_json<T: DeserializeOwned>(self) -> eyre::Result<T>;
}

impl JsonRequest for RequestBuilder {
    async fn send_it(self) -> eyre::Result<()> {
        let response = self.send().await?;

        let url = response.url().to_string();
        let status = response.status();
        let full = response.bytes().await?;

        if status.is_success() {
            Ok(())
        } else {
            let body = String::from_utf8_lossy(&full).into_owned();
            Err(RequestError {
                msg: "request failed".to_string(),
                url,
                status,
                body,
            }
            .into())
        }
    }
    async fn send_it_json<T: DeserializeOwned>(self) -> eyre::Result<T> {
        let response = self.send().await?;

        let url = response.url().to_string();
        let status = response.status();
        let full = response.bytes().await?;

        if status.is_success() {
            match serde_json::from_slice(&full) {
                Ok(parsed) => Ok(parsed),
                Err(error) => {
                    let body = String::from_utf8_lossy(&full).into_owned();
                    let msg = "Failed to parse JSON response".to_string();
                    Err(error).wrap_err(RequestError {
                        msg,
                        url,
                        status,
                        body,
                    })
                }
            }
        } else {
            let body = String::from_utf8_lossy(&full).into_owned();
            Err(RequestError {
                msg: "request failed".to_string(),
                url,
                status,
                body,
            }
            .into())
        }
    }
}
