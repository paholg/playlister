use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Track {
    pub artist: String,
    pub track: String,
}

impl Track {
    pub fn new(artist: String, track: String) -> Track {
        Track { artist, track }
    }

    pub fn as_spotify_query(&self) -> String {
        let Track { track, artist } = self;
        let res = format!("track:{track} artist:{artist}");
        urlencoding::encode(&res).into_owned()
    }

    pub fn as_tidal_query(&self) -> String {
        let res = format!("{} {}", self.artist, self.track);
        urlencoding::encode(&res).into_owned()
    }
}

impl fmt::Display for Track {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "'{}' - '{}'", self.artist, self.track)
    }
}
