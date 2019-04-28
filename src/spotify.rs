use crate::{track::Track, AuthResponse};
use serde::Deserialize;
use serde_json::json;
use std::env;

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
    fn new(track: Track, record: Option<Record>) -> SpotifyTrack {
        SpotifyTrack { track, record }
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
        let mut n_failed = 0;
        let uris: Vec<String> = tracks
            .into_iter()
            .filter_map(|track| match self.search(track.clone()) {
                Ok(spotify_track) => Some(spotify_track),
                Err(e) => {
                    println!("Error in search result for {}: {}", track, e);
                    None
                }
            })
            .filter_map(|spotify_track| match spotify_track.record {
                Some(record) => Some(record.uri),
                None => {
                    println!("Failed to find track for: {}", spotify_track.track);
                    n_failed += 1;
                    None
                }
            })
            .collect();

        println!("Found {} of {} tracks", uris.len(), uris.len() + n_failed);

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

    fn search(&self, track: Track) -> Result<SpotifyTrack, failure::Error> {
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
            .send()?
            .json()?;

        let record = response
            .tracks
            .items
            .get(0)
            .map(|item| Record::new(item.uri.clone(), item.name.clone()));

        Ok(SpotifyTrack::new(track, record))
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
