use crate::{track::Track, AuthResponse};
use futures::stream::{FuturesOrdered, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::{env, iter::FromIterator};
use tracing::{debug, error, info};

struct Record {
    uri: String,
    name: String,
}

impl Record {
    fn new(uri: String, name: String) -> Record {
        Record { uri, name }
    }
}

struct SpotifyTrack {
    track: Track,
    record: Option<Record>,
}

impl SpotifyTrack {
    fn new(track: Track, record: Option<Record>) -> Self {
        Self { track, record }
    }
}

pub async fn run(client: reqwest::Client, tracks: Vec<Track>) {
    if let Err(error) = Spotify::run(client, tracks).await {
        tracing::error!(%error, "Spotify error");
    }
}

struct Spotify {
    access_token: String,
    user_access_token: String,
    client: reqwest::Client,
}

impl Spotify {
    async fn run(client: reqwest::Client, tracks: Vec<Track>) -> eyre::Result<()> {
        Self::new(client).await?.update_playlist(tracks).await
    }

    async fn new(client: reqwest::Client) -> eyre::Result<Self> {
        let access_token = get_app_access_token(&client).await?;
        let user_access_token = get_user_access_token(&client).await?;

        Ok(Self {
            access_token,
            client,
            user_access_token,
        })
    }

    async fn update_playlist<I: IntoIterator<Item = Track>>(&self, tracks: I) -> eyre::Result<()> {
        let mut n_failed = 0;

        let futures = tracks.into_iter().map(|track| self.search(track));

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
            .filter_map(|spotify_track| match spotify_track.record {
                Some(record) => {
                    debug!("Found '{}' to match {}", record.name, spotify_track.track);
                    Some(record.uri)
                }
                None => {
                    debug!("Failed to find track: {}", spotify_track.track);
                    n_failed += 1;
                    None
                }
            })
            .collect::<Vec<_>>();

        info!("Found {} of {} tracks", uris.len(), uris.len() + n_failed);

        let body = json!({ "uris": uris });

        self.client
            .put(&format!(
                "https://api.spotify.com/v1/playlists/{}/tracks",
                env::var("SPOTIFY_PLAYLIST_ID")?
            ))
            .bearer_auth(&self.user_access_token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn search(&self, track: Track) -> eyre::Result<SpotifyTrack> {
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
            .client
            .get("https://api.spotify.com/v1/search")
            .bearer_auth(&self.access_token)
            .query(&[
                ("type", "track"),
                ("q", &track.as_spotify_query()),
                ("limit", "1"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let record = response
            .tracks
            .items
            .get(0)
            .map(|item| Record::new(item.uri.clone(), item.name.clone()));

        Ok(SpotifyTrack::new(track, record))
    }
}

async fn get_app_access_token(client: &reqwest::Client) -> eyre::Result<String> {
    get_access_token(client, "grant_type=client_credentials".into()).await
}

async fn get_user_access_token(client: &reqwest::Client) -> eyre::Result<String> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}",
        env::var("SPOTIFY_REFRESH_TOKEN")?
    );
    get_access_token(client, body).await
}

async fn get_access_token(client: &reqwest::Client, body: String) -> eyre::Result<String> {
    let response: AuthResponse = client
        .post("https://accounts.spotify.com/api/token")
        .basic_auth(
            env::var("SPOTIFY_CLIENT_ID")?,
            Some(env::var("SPOTIFY_CLIENT_SECRET")?),
        )
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(response.access_token)
}
