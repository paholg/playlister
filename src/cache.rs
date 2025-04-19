use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::{spotify, tidal, track::Track};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Item {
    tidal: Option<tidal::Record>,
    spotify: Option<spotify::Record>,
}

#[derive(Default, Clone)]
pub struct Cache {
    map: Arc<DashMap<Track, Item>>,
}

impl<'de> Deserialize<'de> for Cache {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map = Vec::deserialize(deserializer)?.into_iter().collect();

        Ok(Self { map: Arc::new(map) })
    }
}

impl Serialize for Cache {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let vec: Vec<_> = self
            .map
            .iter()
            .map(|entry| {
                let (k, v) = entry.pair();
                (k.clone(), v.clone())
            })
            .collect();
        vec.serialize(serializer)
    }
}

impl Cache {
    pub fn set_spotify(&self, track: Track, record: spotify::Record) {
        self.map.entry(track).or_default().spotify = Some(record);
    }

    pub fn set_tidal(&self, track: Track, record: tidal::Record) {
        self.map.entry(track).or_default().tidal = Some(record);
    }

    pub fn get_spotify(&self, track: &Track) -> Option<spotify::Record> {
        self.map.get(track).and_then(|item| item.spotify.clone())
    }

    pub fn get_tidal(&self, track: &Track) -> Option<tidal::Record> {
        self.map.get(track).and_then(|item| item.tidal.clone())
    }

    pub fn trim(&self, tracks: &[Track]) {
        let tracks = tracks.iter().collect::<HashSet<_>>();
        self.map.retain(|k, _v| tracks.contains(k));
    }
}
