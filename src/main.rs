use std::path::PathBuf;

use config::Environment;
use playlister::{
    Data,
    cache::Cache,
    reddit,
    spotify::{self, Spotify},
    tidal::{self, Tidal},
    track::Track,
};
use serde::Deserialize;
use tracing::{Level, error, info};

#[derive(Deserialize, Debug)]
pub struct Settings {
    cache_dir: Option<PathBuf>,
    reddit: reddit::Settings,
    spotify: spotify::Settings,
    tidal: tidal::Settings,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let log_subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_file(true)
        .with_line_number(true);
    let _ = dotenv::dotenv();

    log_subscriber.init();
    run().await?;

    Ok(())
}

async fn run() -> eyre::Result<()> {
    info!("Beginning update");

    let config = config::Config::builder()
        .add_source(Environment::default().separator("__"))
        .build()?;
    let settings: Settings = config.try_deserialize()?;

    let cache = if let Some(path) = &settings.cache_dir {
        if std::fs::exists(path)? {
            serde_json::from_str(&std::fs::read_to_string(path)?)?
        } else {
            Cache::default()
        }
    } else {
        Cache::default()
    };

    let listentothis_regex = regex::Regex::new(r"(.*?)\s+[-–—\s]+\s+(.*?)\s*[\(\[]")?;

    let client = reqwest::Client::new();
    let tracks: Vec<Track> = reddit::Reddit::new(settings.reddit, client.clone())
        .await?
        .tracks("r/listentothis", listentothis_regex)
        .await?;

    let mut handles = Vec::new();

    let spotify: Data<Spotify> = Data::new(&cache, &client, settings.spotify, &tracks);
    let spotify = tokio::spawn(spotify.run());
    handles.push(spotify);

    let tidal: Data<Tidal> = Data::new(&cache, &client, settings.tidal, &tracks);
    let tidal = tokio::spawn(tidal.run());
    handles.push(tidal);

    for handle in handles {
        let result = handle.await;

        if let Err(error) = result {
            error!(%error, "Join error")
        }
    }

    if let Some(path) = settings.cache_dir {
        cache.trim(&tracks);
        let ser = serde_json::to_string(&cache)?;
        std::fs::create_dir_all(&path)?;
        std::fs::write(path, ser)?;
    }
    info!("Update complete");
    Ok(())
}
