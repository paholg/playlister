use std::fmt;

#[derive(Clone, Debug)]
pub struct Track {
    artist: String,
    track: String,
}

impl Track {
    pub fn new(artist: String, track: String) -> Track {
        Track { artist, track }
    }

    pub fn as_spotify_query(&self) -> String {
        let res = format!("{} {}", self.artist, self.track);
        res
    }

    pub fn as_tidal_query(&self) -> String {
        self.as_spotify_query()
    }
}

impl fmt::Display for Track {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "'{}' - '{}'", self.artist, self.track)
    }
}
