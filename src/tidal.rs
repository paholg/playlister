use crate::{AuthResponse, Data, JsonRequest, Record, Secret, Service, track::Track};
use futures::{StreamExt, stream::FuturesOrdered};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tracing::debug;

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
    const NAME: &'static str = "tidal";
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
    async fn update_playlist(&self) -> eyre::Result<()> {
        let ids = self
            .data
            .search_all(|t| self.search(t))
            .await
            .into_iter()
            .map(|r| r.id)
            .collect::<Vec<_>>();

        self.clear_playlist().await?;
        self.add_tracks_to_playlist(ids).await?;

        Ok(())
    }

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

    async fn search(&self, track: &Track) -> eyre::Result<Option<Record>> {
        #[derive(Deserialize, Debug)]
        struct TrackResponse {
            #[serde(default)]
            included: Vec<TrackIncluded>,
        }

        #[derive(Deserialize, Debug)]
        struct TrackIncluded {
            id: String,
            #[serde(rename = "type")]
            ty: String,
            attributes: TrackAttributes,
            relationships: Relationships,
        }

        #[derive(Deserialize, Debug)]
        struct TrackAttributes {
            title: String,
        }

        #[derive(Deserialize, Debug)]
        struct Relationships {
            artists: Relationship,
        }

        #[derive(Deserialize, Debug)]
        struct Relationship {
            links: Links,
        }

        #[derive(Deserialize, Debug)]
        struct Links {
            #[serde(rename = "self")]
            sel: String,
        }

        debug!("searching playlist");
        let response: TrackResponse = self
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

        let Some(included) = response.included.into_iter().find(|inc| inc.ty == "tracks") else {
            return Ok(None);
        };

        // To get the artist names, we have to query the link they sent us, which gives us the
        // artist ids. Then we have to query for each id. Ugggh.
        #[derive(Deserialize, Debug)]
        struct ArtistIdResponse {
            data: Vec<ArtistId>,
        }

        #[derive(Deserialize, Debug)]
        struct ArtistId {
            id: String,
        }

        let artist_data: ArtistIdResponse = self
            .data
            .client
            .get(format!(
                "https://openapi.tidal.com/v2{}",
                included.relationships.artists.links.sel
            ))
            .bearer_auth(self.app_access_token.expose_secret())
            .send_it_json()
            .await?;

        #[derive(Deserialize, Debug)]
        struct ArtistDataResponse {
            data: ArtistData,
        }

        #[derive(Deserialize, Debug)]
        struct ArtistData {
            attributes: ArtistAttributes,
        }

        #[derive(Deserialize, Debug)]
        struct ArtistAttributes {
            name: String,
        }

        let futures = artist_data.data.into_iter().map(|datum| {
            let id = datum.id;

            self.data
                .client
                .get(format!("https://openapi.tidal.com/v2/artists/{id}"))
                .query(&[("countryCode", "US")])
                .bearer_auth(self.app_access_token.expose_secret())
                .send_it_json::<ArtistDataResponse>()
        });
        let results = FuturesOrdered::from_iter(futures).collect::<Vec<_>>().await;

        let artists = results
            .into_iter()
            .map(|r| r.map(|response| response.data.attributes.name))
            .collect::<eyre::Result<Vec<_>>>()?;

        Ok(Some(Record {
            id: included.id,
            title: included.attributes.title,
            artists,
        }))
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
