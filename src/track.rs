use std::fmt;

#[derive(Debug)]
pub struct Track {
    artist: String,
    track: String,
}

impl Track {
    pub fn new(artist: String, track: String) -> Track {
        Track { artist, track }
    }

    pub fn as_spotify_query(&self) -> String {
        let res = format!("track:\"{}\" artist:\"{}\"", self.track, self.artist);
        res
    }
}

impl fmt::Display for Track {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} - {}", self.artist, self.track)
    }
}
