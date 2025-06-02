use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Track {
    pub artist: String,
    pub title: String,
}

impl Track {
    pub fn new(artist: String, title: String) -> Track {
        Track { artist, title }
    }

    pub fn as_spotify_query(&self) -> String {
        let Track { title, artist } = self;
        format!("track:{title} artist:{artist}")
    }

    pub fn as_tidal_query(&self) -> String {
        format!("{} {}", self.artist, self.title)
    }
}

impl fmt::Display for Track {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "'{}' - '{}'", self.artist, self.title)
    }
}
