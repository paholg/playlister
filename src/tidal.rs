use crate::{AuthResponse, Data, JsonRequest, Secret, Service, track::Track};
use futures::stream::{FuturesOrdered, StreamExt};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::iter::FromIterator;
use tracing::{Span, debug, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    id: String,
}

impl Record {
    fn new(id: String) -> Record {
        Record { id }
    }
}

#[derive(Deserialize, Debug)]
pub struct Settings {
    client_id: String,
    client_secret: Secret<String>,
    refresh_token: Secret<String>,
    playlist_id: String,
}

pub struct Tidal {
    data: Data<Self>,
    app_access_token: Secret<String>,
    user_access_token: Secret<String>,
}

impl Service for Tidal {
    type Settings = Settings;

    async fn new(data: Data<Self>) -> eyre::Result<Self> {
        let app_access_token = data.get_app_access_token().await?;
        let user_access_token = data.get_user_access_token().await?;

        Ok(Self {
            data,
            app_access_token,
            user_access_token,
        })
    }

    async fn run(&self) -> eyre::Result<()> {
        self.update_playlist().await
    }
}

#[derive(Serialize, Deserialize)]
struct PlaylistItem {
    id: String,
    #[serde(rename = "type")]
    ty: String,
    meta: PlaylistItemMeta,
}

#[derive(Serialize, Deserialize)]
struct PlaylistItemMeta {
    #[serde(rename = "itemId")]
    item_id: String,
}

impl Tidal {
    fn playlist_id(&self) -> &str {
        &self.data.settings.playlist_id
    }

    async fn get_playlist(&self) -> eyre::Result<Vec<PlaylistItem>> {
        #[derive(Deserialize)]
        struct Response {
            data: Vec<PlaylistItem>,
            links: Links,
        }

        #[derive(Deserialize)]
        struct Links {
            next: Option<String>,
        }

        debug!("getting playlist");
        let response: Response = self
            .data
            .client
            .get(format!(
                "https://openapi.tidal.com/v2/playlists/{}/relationships/items",
                self.playlist_id()
            ))
            .query(&[("countryCode", "US")])
            .bearer_auth(self.app_access_token.expose_secret())
            .send_it_json()
            .await?;
        let mut cursor: Option<String> = response.links.next;
        let mut result = response.data;

        while let Some(path) = cursor {
            debug!("paging playlist");
            let response: Response = self
                .data
                .client
                .get(format!("https://openapi.tidal.com/v2{path}",))
                .bearer_auth(self.app_access_token.expose_secret())
                .send_it_json()
                .await?;
            result.extend(response.data);
            cursor = response.links.next;
        }
        Ok(result)
    }

    async fn clear_playlist(&self) -> eyre::Result<()> {
        let playlist = self.get_playlist().await?;

        #[derive(Serialize)]
        struct Request {
            data: Vec<PlaylistItem>,
        }

        let requests: Vec<Request> = playlist
            .into_iter()
            .chunks(20)
            .into_iter()
            .map(|chunk| Request {
                data: chunk.into_iter().collect(),
            })
            .collect();

        for request in requests {
            let request_json = serde_json::to_string(&request)?;
            debug!(%request_json, "clearing playlist");
            self.data
                .client
                .delete(format!(
                    "https://openapi.tidal.com/v2/playlists/{}/relationships/items",
                    self.playlist_id()
                ))
                .bearer_auth(self.user_access_token.expose_secret())
                .json(&request)
                .send_it()
                .await?;
        }

        Ok(())
    }

    async fn add_tracks_to_playlist(&self, tracks: Vec<String>) -> eyre::Result<()> {
        #[derive(Serialize)]
        struct Request {
            data: Vec<RequestData>,
        }

        #[derive(Serialize)]
        struct RequestData {
            id: String,
            #[serde(rename = "type")]
            ty: &'static str,
        }

        let requests: Vec<Request> = tracks
            .into_iter()
            .chunks(20)
            .into_iter()
            .map(|chunk| Request {
                data: chunk
                    .into_iter()
                    .map(|id| RequestData { id, ty: "tracks" })
                    .collect(),
            })
            .collect();

        for request in requests {
            debug!("adding tracks to playlist");
            self.data
                .client
                .post(format!(
                    "https://openapi.tidal.com/v2/playlists/{}/relationships/items",
                    self.playlist_id()
                ))
                .bearer_auth(self.user_access_token.expose_secret())
                .json(&request)
                .send_it()
                .await?;
        }

        Ok(())
    }

    async fn update_playlist(&self) -> eyre::Result<()> {
        let futures = self.data.tracks.iter().map(|track| async {
            if let Some(record) = self.data.cache.get_tidal(track) {
                Ok(Some(record))
            } else {
                let result = self.search(track).await;

                if let Ok(Some(record)) = &result {
                    self.data.cache.set_tidal(track.clone(), record.clone());
                }
                result
            }
        });

        let ids = FuturesOrdered::from_iter(futures)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|result| match result {
                Ok(Some(record)) => Some(record.id),
                Ok(None) => None,
                Err(error) => {
                    error!(%error, "Error in search result");
                    None
                }
            })
            .collect::<Vec<_>>();

        Span::current().record("found", ids.len());

        self.clear_playlist().await?;
        self.add_tracks_to_playlist(ids).await?;

        Ok(())
    }

    async fn search(&self, track: &Track) -> eyre::Result<Option<Record>> {
        #[derive(Deserialize, Debug)]
        struct Response {
            #[serde(default)]
            included: Vec<Included>,
        }

        #[derive(Deserialize, Debug)]
        struct Included {
            id: String,
            #[serde(rename = "type")]
            ty: String,
        }

        debug!("searching playlist");
        let response: Response = self
            .data
            .client
            .get(format!(
                "https://openapi.tidal.com/v2/searchResults/{}",
                track.as_tidal_query()
            ))
            .bearer_auth(self.app_access_token.expose_secret())
            .query(&[("countryCode", "US"), ("include", "tracks")])
            .send_it_json()
            .await?;

        let record = response
            .included
            .into_iter()
            .find(|inc| inc.ty == "tracks")
            .map(|item| Record::new(item.id));

        Ok(record)
    }
}

impl Data<Tidal> {
    async fn get_app_access_token(&self) -> eyre::Result<Secret<String>> {
        self.get_access_token(&[("grant_type", "client_credentials")])
            .await
    }

    async fn get_user_access_token(&self) -> eyre::Result<Secret<String>> {
        self.get_access_token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", self.settings.refresh_token.expose_secret()),
        ])
        .await
    }

    async fn get_access_token(&self, body: &[(&str, &str)]) -> eyre::Result<Secret<String>> {
        let response: AuthResponse = self
            .client
            .post("https://auth.tidal.com/v1/oauth2/token")
            .basic_auth(
                &self.settings.client_id,
                Some(self.settings.client_secret.expose_secret()),
            )
            .form(&body)
            .send_it_json()
            .await?;

        Ok(response.access_token)
    }
}
