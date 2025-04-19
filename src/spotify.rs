use crate::{AuthResponse, Data, JsonRequest, Secret, Service, track::Track};
use futures::stream::{FuturesOrdered, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::iter::FromIterator;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    uri: String,
    name: String,
}

impl Record {
    fn new(uri: String, name: String) -> Record {
        Record { uri, name }
    }
}

#[derive(Deserialize, Debug)]
pub struct Settings {
    client_id: String,
    client_secret: Secret<String>,
    refresh_token: Secret<String>,
    playlist_id: String,
}

pub struct Spotify {
    data: Data<Self>,
    app_access_token: Secret<String>,
    user_access_token: Secret<String>,
}

impl Service for Spotify {
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

impl Spotify {
    async fn update_playlist(&self) -> eyre::Result<()> {
        let mut n_failed = 0;

        let futures = self.data.tracks.iter().map(|track| async {
            if let Some(record) = self.data.cache.get_spotify(track) {
                Ok(Some(record))
            } else {
                let result = self.search(track).await;

                if let Ok(Some(record)) = &result {
                    self.data.cache.set_spotify(track.clone(), record.clone());
                }
                result
            }
        });

        let uris = FuturesOrdered::from_iter(futures)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|result| match result {
                Ok(spotify_track) => Some(spotify_track),
                Err(error) => {
                    error!(%error, "Error in search result");
                    None
                }
            })
            .filter_map(|record| match record {
                Some(record) => Some(record.uri),
                None => {
                    n_failed += 1;
                    None
                }
            })
            .collect::<Vec<_>>();

        info!("Found {} of {} tracks", uris.len(), uris.len() + n_failed);

        let body = json!({ "uris": uris });

        self.data
            .client
            .put(format!(
                "https://api.spotify.com/v1/playlists/{}/tracks",
                self.data.settings.playlist_id
            ))
            .bearer_auth(self.user_access_token.expose_secret())
            .json(&body)
            .send_it()
            .await?;

        Ok(())
    }

    async fn search(&self, track: &Track) -> eyre::Result<Option<Record>> {
        #[derive(Deserialize, Debug)]
        struct Response {
            tracks: Items,
        }

        #[derive(Deserialize, Debug)]
        struct Items {
            items: Vec<Item>,
        }

        #[derive(Deserialize, Debug)]
        struct Item {
            uri: String,
            name: String,
        }

        let response: Response = self
            .data
            .client
            .get("https://api.spotify.com/v1/search")
            .bearer_auth(self.app_access_token.expose_secret())
            .query(&[
                ("type", "track"),
                ("q", &track.as_spotify_query()),
                ("limit", "1"),
            ])
            .send_it_json()
            .await?;

        let record = response
            .tracks
            .items
            .first()
            .map(|item| Record::new(item.uri.clone(), item.name.clone()));

        Ok(record)
    }
}

impl Data<Spotify> {
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
            .post("https://accounts.spotify.com/api/token")
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
