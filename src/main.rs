use serde::Deserialize;
use tracing::{error, info, Level};

use crate::track::Track;

mod reddit;
mod spotify;
mod track;

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    dotenv::dotenv()?;

    perform().await?;

    Ok(())
}

async fn perform() -> eyre::Result<()> {
    let listentothis_regex = regex::Regex::new(r"(.*?)\s+[-–—\s]+\s+(.*?)\s*[\(\[]")?;

    info!("Beginning update");
    let client = reqwest::Client::new();
    let tracks: Vec<Track> = reddit::Reddit::new(client.clone())
        .await?
        .tracks("r/listentothis", listentothis_regex)
        .await?
        .collect();

    let mut handles = Vec::new();

    let client_clone = client.clone();
    let tracks_clone = tracks.clone();
    let spotify = tokio::spawn(spotify::run(client_clone, tracks_clone));
    handles.push(spotify);

    // let client_clone = client.clone();
    // let tracks_clone = tracks.clone();
    // let tidal = tokio::spawn(async {
    //     // TODO
    // });
    // handles.push(tidal);

    for handle in handles {
        let result = handle.await;
        match result {
            Err(error) => {
                error!(%error, "Join error")
            }
            _ => {}
        }
    }

    info!("Update complete");
    Ok(())
}
