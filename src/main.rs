use std::{path::PathBuf, str::FromStr};

use config::Environment;
use playlister::{
    reddit,
    spotify::{self, Spotify},
    tidal::{self, Tidal},
};
use serde::{Deserialize, Deserializer};
use tokio::{runtime, task::JoinSet};
use tracing::{Level, field, info, info_span};
use tracing_subscriber::fmt::format::FmtSpan;

#[derive(Deserialize, Debug)]
pub struct Settings {
    #[serde(default = "info", deserialize_with = "de_level")]
    log_level: Level,
    cache_dir: Option<PathBuf>,
    reddit: reddit::Settings,
    spotify: Option<spotify::Settings>,
    tidal: Option<tidal::Settings>,
}

fn info() -> Level {
    Level::INFO
}

fn de_level<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Level, D::Error> {
    let s = String::deserialize(deserializer)?;
    Level::from_str(&s).map_err(serde::de::Error::custom)
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let _ = dotenv::dotenv();

    let config = config::Config::builder()
        .add_source(Environment::default().separator("__"))
        .build()?;
    let settings: Settings = config.try_deserialize()?;

    tracing_subscriber::fmt()
        .with_max_level(settings.log_level)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(run(settings))?;

    Ok(())
}

#[tracing::instrument(skip(settings))]
async fn run(settings: Settings) -> eyre::Result<()> {
    info!("Beginning update");

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

    let mut set = JoinSet::new();

    if let Some(spotify_settings) = settings.spotify {
        let fut = playlister::run::<Spotify>(
            settings.cache_dir.clone(),
            spotify_settings,
            tracks.clone(),
            client.clone(),
        );
        set.spawn(fut);
    }
    if let Some(tidal_settings) = settings.tidal {
        let fut = playlister::run::<Tidal>(settings.cache_dir, tidal_settings, tracks, client);
        set.spawn(fut);
    }

    set.join_all().await;
    Ok(())
}
