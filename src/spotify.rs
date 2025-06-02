use crate::{AuthResponse, Data, JsonRequest, Record, Secret, Service, track::Track};
use serde::Deserialize;
use serde_json::json;

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
    const NAME: &'static str = "spotify";
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
        let uris = self
            .data
            .search_all(|t| self.search(t))
            .await
            .into_iter()
            .map(|r| r.id)
            .collect::<Vec<_>>();

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
            artists: Vec<Artist>,
        }

        #[derive(Deserialize, Debug)]
        struct Artist {
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

        let record = response.tracks.items.into_iter().next().map(|item| Record {
            id: item.uri,
            title: item.name,
            artists: item.artists.into_iter().map(|artist| artist.name).collect(),
        });

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
