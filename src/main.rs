use lambda_runtime::error::HandlerError;
use lambda_runtime::{lambda, Context};
use log::{error, info};
use serde::Deserialize;

mod reddit;
mod spotify;
mod track;

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
}

fn main() -> Result<(), failure::Error> {
    simple_logger::init_with_level(log::Level::Info)?;
    dotenv::dotenv().ok();

    if cfg!(target_env = "musl") {
        lambda!(handler);
    } else {
        perform()?;
    }

    Ok(())
}

fn perform() -> Result<(), failure::Error> {
    info!("Beginning update");
    let client = reqwest::Client::new();
    let tracks = reddit::Reddit::new(&client)?.listentothis_hot()?;
    spotify::Spotify::new(&client)?.update_playlist(tracks)?;
    info!("Update complete");
    Ok(())
}

fn handler(_e: serde_json::Value, _c: Context) -> Result<(), HandlerError> {
    let result = perform();
    if let Err(e) = &result {
        error!("Failed: {}", e);
    }
    result?;

    Ok(())
}
