use std::{path::PathBuf, str::FromStr};

use config::Environment;
use playlister::{
    Data,
    cache::Cache,
    reddit,
    spotify::{self, Spotify},
    tidal::{self, Tidal},
};
use serde::{Deserialize, Deserializer};
use tracing::{Level, error, field, info, info_span};
use tracing_subscriber::fmt::{self, format::FmtSpan};

#[derive(Deserialize, Debug)]
pub struct Settings {
    #[serde(default = "info", deserialize_with = "de_level")]
    log_level: Level,
    cache_dir: Option<PathBuf>,
    reddit: reddit::Settings,
    spotify: spotify::Settings,
    tidal: tidal::Settings,
}

fn info() -> Level {
    Level::INFO
}

fn de_level<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Level, D::Error> {
    let s = <&str>::deserialize(deserializer)?;
    Level::from_str(s).map_err(|e| serde::de::Error::custom(e))
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let _ = dotenv::dotenv();

    let config = config::Config::builder()
        .add_source(Environment::default().separator("__"))
        .build()?;
    let settings: Settings = config.try_deserialize()?;

    tracing_subscriber::fmt()
        .with_max_level(settings.log_level)
        .with_span_events(FmtSpan::CLOSE)
        .with_timer(fmt::time::uptime())
        .init();

    run(settings).await?;

    Ok(())
}

#[tracing::instrument(skip(settings))]
async fn run(settings: Settings) -> eyre::Result<()> {
    info!("Beginning update");

    let cache_path = settings.cache_dir.map(|mut dir| {
        dir.push("cache.json");
        dir
    });

    let cache = if let Some(path) = &cache_path {
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

    let tracks = {
        let span = info_span!("reddit", count = field::Empty);
        let _enter = span.enter();
        let tracks = reddit::Reddit::new(settings.reddit, client.clone())
            .await?
            .tracks("r/listentothis", listentothis_regex)
            .await?;
        span.record("count", tracks.len());
        tracks
    };

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

    if let Some(path) = cache_path {
        cache.trim(&tracks);
        let ser = serde_json::to_string(&cache)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, ser)?;
    }
    Ok(())
}
