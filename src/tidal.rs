use crate::{track::Track, AuthResponse};
use futures::stream::{FuturesOrdered, StreamExt};
use itertools::Itertools;
use serde::Deserialize;
use std::{env, iter::FromIterator};
use tracing::{debug, error, info};

struct Record {
    id: u64,
    name: String,
}

impl Record {
    fn new(id: u64, name: String) -> Record {
        Record { id, name }
    }
}

struct TidalTrack {
    track: Track,
    record: Option<Record>,
}

impl TidalTrack {
    fn new(track: Track, record: Option<Record>) -> Self {
        Self { track, record }
    }
}

pub async fn run(client: reqwest::Client, tracks: Vec<Track>) {
    if let Err(error) = Tidal::run(client, tracks).await {
        tracing::error!(%error, "Tidal error");
    }
}

struct Tidal {
    client: reqwest::Client,
    user_access_token: String,
}

impl Tidal {
    async fn run(client: reqwest::Client, tracks: Vec<Track>) -> eyre::Result<()> {
        Self::new(client).await?.update_playlist(tracks).await
    }

    async fn new(client: reqwest::Client) -> eyre::Result<Self> {
        let user_access_token = get_user_access_token(&client).await?;

        Ok(Self {
            client,
            user_access_token,
        })
    }

    async fn clear_playlist(&self) -> eyre::Result<()> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Playlist {
            number_of_tracks: u32,
        }

        let playlist: Playlist = self
            .client
            .get(&format!(
                "https://api.tidal.com/v1/playlists/{}",
                env::var("TIDAL_PLAYLIST_ID")?
            ))
            .query(&[("countryCode", "US")])
            // .header(reqwest::header::IF_NONE_MATCH, "*")
            .bearer_auth(&self.user_access_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // Deleting back to front appears to be much faster.
        for i in (0..playlist.number_of_tracks).rev() {
            self.client
                .delete(&format!(
                    "https://api.tidal.com/v1/playlists/{}/items/{i}",
                    env::var("TIDAL_PLAYLIST_ID")?
                ))
                .query(&[("countryCode", "US")])
                .header(reqwest::header::IF_NONE_MATCH, "*")
                .bearer_auth(&self.user_access_token)
                .send()
                .await?
                .error_for_status()?;
        }

        Ok(())
    }

    async fn add_tracks_to_playlist(&self, tracks: Vec<u64>) -> eyre::Result<()> {
        let track_ids = Itertools::join(&mut tracks.iter(), ",");
        // let track_ids = tracks[0].to_string();

        let body = format!("onDupes=SKIP&onArtifactNotFound=SKIP&trackIds={track_ids}");

        // NEED SessionId for query?
        self.client
            .post(&format!(
                "https://api.tidal.com/v1/playlists/{}/items",
                env::var("TIDAL_PLAYLIST_ID")?
            ))
            .query(&[("limit", "100"), ("countryCode", "US")])
            .header(reqwest::header::IF_NONE_MATCH, "*")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .bearer_auth(&self.user_access_token)
            .body(body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn update_playlist<I: IntoIterator<Item = Track>>(&self, tracks: I) -> eyre::Result<()> {
        let mut n_failed = 0;

        let futures = tracks.into_iter().map(|track| self.search(track));

        let ids = FuturesOrdered::from_iter(futures)
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
            .filter_map(|tidal_track| match tidal_track.record {
                Some(record) => {
                    debug!("Found '{}' to match {}", record.name, tidal_track.track);
                    Some(record.id)
                }
                None => {
                    debug!("Failed to find track: {}", tidal_track.track);
                    n_failed += 1;
                    None
                }
            })
            .collect::<Vec<_>>();

        info!("Found {} of {} tracks", ids.len(), ids.len() + n_failed);

        self.clear_playlist().await?;
        self.add_tracks_to_playlist(ids).await?;

        Ok(())
    }

    async fn search(&self, track: Track) -> eyre::Result<TidalTrack> {
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
            id: u64,
            title: String,
        }

        let response: Response = self
            .client
            .get("https://api.tidal.com/v1/search")
            .bearer_auth(&self.user_access_token)
            .query(&[
                ("types", "TRACKS"),
                ("query", &track.as_tidal_query()),
                ("limit", "1"),
                ("offset", "0"),
                ("countryCode", "US"),
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
            .map(|item| Record::new(item.id, item.title.clone()));

        Ok(TidalTrack::new(track, record))
    }
}

async fn get_user_access_token(client: &reqwest::Client) -> eyre::Result<String> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}",
        env::var("TIDAL_REFRESH_TOKEN")?
    );
    get_access_token(client, body).await
}

async fn get_access_token(client: &reqwest::Client, body: String) -> eyre::Result<String> {
    let response: AuthResponse = client
        .post("https://auth.tidal.com/v1/oauth2/token")
        .basic_auth(
            env::var("TIDAL_CLIENT_ID")?,
            Some(env::var("TIDAL_CLIENT_SECRET")?),
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
