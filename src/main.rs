use serde::Deserialize;

mod reddit;
mod spotify;
mod track;

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
}

fn main() -> Result<(), failure::Error> {
    dotenv::dotenv().ok();

    let client = reqwest::Client::new();
    let tracks = reddit::Reddit::new(&client)?.listentothis_hot()?;
    spotify::Spotify::new(&client)?.update_playlist(tracks)?;

    Ok(())
}
