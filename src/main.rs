use serde::Deserialize;
use tracing::info;

mod reddit;
mod spotify;
mod track;

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt().init();
    dotenv::dotenv()?;

    perform().await?;

    Ok(())
}

async fn perform() -> eyre::Result<()> {
    let listentothis_regex = regex::Regex::new(r"(.*?)\s+[-–—\s]+\s+(.*?)\s*[\(\[]")?;

    info!("Beginning update");
    let client = reqwest::Client::new();
    let tracks = reddit::Reddit::new(client.clone())
        .await?
        .tracks("r/listentothis", listentothis_regex)
        .await?;
    spotify::Spotify::new(client)
        .await?
        .update_playlist(tracks)
        .await?;
    info!("Update complete");
    Ok(())
}
