use crate::{track::Track, AuthResponse};
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;

struct SpotifyTrack {
    track: Track,
    uri: Option<String>,
}

impl SpotifyTrack {
    fn new(track: Track, uri: Option<String>) -> SpotifyTrack {
        SpotifyTrack { track, uri }
    }
}

pub struct Spotify<'a> {
    access_token: String,
    user_access_token: String,
    client: &'a reqwest::Client,
}

impl<'a> Spotify<'a> {
    pub fn new(client: &reqwest::Client) -> Result<Spotify, failure::Error> {
        let access_token = get_app_access_token(client)?;
        let user_access_token = get_user_access_token(client)?;

        Ok(Spotify {
            access_token,
            client,
            user_access_token,
        })
    }

    pub fn update_playlist<I: IntoIterator<Item = Track>>(
        &self,
        tracks: I,
    ) -> Result<(), failure::Error> {
        let uris: Vec<String> = tracks
            .into_iter()
            .filter_map(|track| match self.search(&track) {
                Ok(uri) => Some(SpotifyTrack::new(track, uri)),
                Err(e) => {
                    println!("Error in search result for {}: {}", track, e);
                    None
                }
            })
            .filter_map(|spotify_track| {
                if spotify_track.uri.is_none() {
                    println!("Failed to find track for {}", spotify_track.track);
                }
                spotify_track.uri
            })
            .collect();

        let body = json!({ "uris": uris });

        self.client
            .put(&format!(
                "https://api.spotify.com/v1/playlists/{}/tracks",
                env::var("SPOTIFY_PLAYLIST_ID")?
            ))
            .bearer_auth(&self.user_access_token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&body)
            .send()?
            .error_for_status()?;

        Ok(())
    }

    fn search(&self, track: &Track) -> Result<Option<String>, failure::Error> {
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
            .send()?
            .json()?;

        let uri = response.tracks.items.get(0).map(|item| item.uri.clone());
        Ok(uri)
    }
}

fn get_app_access_token(client: &reqwest::Client) -> Result<String, failure::Error> {
    get_access_token(client, "grant_type=client_credentials".into())
}

fn get_user_access_token(client: &reqwest::Client) -> Result<String, failure::Error> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}",
        env::var("SPOTIFY_REFRESH_TOKEN")?
    );
    get_access_token(client, body)
}

fn get_access_token(client: &reqwest::Client, body: String) -> Result<String, failure::Error> {
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
        .send()?
        .json()?;

    Ok(response.access_token)
}
