use eyre::Report;
use lambda_runtime::Context;
use serde::Deserialize;
use serde_json::json;
use tracing::{error, info, Level};

use crate::track::Track;

pub mod reddit;
pub mod spotify;
pub mod tidal;
pub mod track;

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let log_subscriber = tracing_subscriber::fmt().with_max_level(Level::INFO);
    let _ = dotenv::dotenv();

    if cfg!(target_env = "musl") {
        log_subscriber.with_ansi(false).init();
        let func = lambda_runtime::handler_fn(lambda_handler);

        // TODO: Improve error conversion
        lambda_runtime::run(func)
            .await
            .map_err(|e| Report::msg(e.to_string()))?;
    } else {
        log_subscriber.init();
        perform().await?;
    }

    Ok(())
}

async fn lambda_handler(
    _event: serde_json::Value,
    _c: Context,
) -> Result<serde_json::Value, lambda_runtime::Error> {
    let result = perform().await;

    if let Err(e) = &result {
        error!("Failed: {}", e);
    }
    result?;

    Ok(json!({}))
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

    let client_clone = client.clone();
    let tracks_clone = tracks.clone();
    let tidal = tokio::spawn(tidal::run(client_clone, tracks_clone));
    handles.push(tidal);

    for handle in handles {
        let result = handle.await;

        if let Err(error) = result {
            error!(%error, "Join error")
        }
    }
    info!("Update complete");
    Ok(())
}
