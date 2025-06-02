use std::{
    fmt,
    path::{Path, PathBuf},
};

use cache::Cache;
use data::Data;
use eyre::Context;
use reqwest::{RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::{error, field};
use track::Track;

pub mod cache;
pub mod data;
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

/// A record, as the result of a service-specific search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// Whatever the given service uses as an id. For tidal, this is a number.
    /// For spotify, it's a uri.
    id: String,
    title: String,
    artists: Vec<String>,
}

#[allow(async_fn_in_trait)]
pub trait Service: Sized {
    const NAME: &'static str;
    type Settings;

    async fn new(data: Data<Self>) -> eyre::Result<Self>;
    async fn run(&self) -> eyre::Result<()>;
}
#[tracing::instrument(skip_all, fields(service = S::NAME, found = field::Empty, cache_hits = field::Empty, filtered = field::Empty))]
pub async fn run<S: Service>(
    cache_dir: Option<PathBuf>,
    settings: S::Settings,
    tracks: Vec<Track>,
    client: reqwest::Client,
) {
    fn load_cache(path: Option<&Path>) -> eyre::Result<Cache> {
        let Some(p) = path else {
            return Ok(Cache::default());
        };
        let cache = serde_json::from_str(&std::fs::read_to_string(p)?)?;
        Ok(cache)
    }

    fn save_cache(path: &Path, cache: &Cache) -> eyre::Result<()> {
        let ser = serde_json::to_string(&cache)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, ser)?;
        Ok(())
    }

    let cache_path = cache_dir.map(|mut dir| {
        dir.push(format!("{}.json", S::NAME));
        dir
    });

    let cache: Cache = match load_cache(cache_path.as_deref()) {
        Ok(c) => {
            c.trim(&tracks);
            c
        }
        Err(error) => {
            error!(%error, "Failed to load cache");
            Cache::default()
        }
    };
    let data: Data<S> = Data::new(&cache, &client, settings, &tracks);
    let client = match S::new(data).await {
        Ok(client) => client,
        Err(error) => {
            error!(%error, "failed to create client");
            return;
        }
    };

    if let Err(error) = client.run().await {
        error!(%error, "failed to update playlist");
    }

    if let Some(path) = cache_path {
        cache.trim(&tracks);
        if let Err(error) = save_cache(&path, &cache) {
            error!(%error, "failed to save cache");
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
