use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;
use futures::{StreamExt, stream::FuturesOrdered};
use serde::{Deserialize, Serialize};
use strsim::normalized_damerau_levenshtein;
use tracing::{Span, error};

use crate::{Record, track::Track};

#[derive(Default)]
pub struct Cache {
    map: Arc<DashMap<Track, Option<Record>>>,
}

pub struct CacheResult {
    pub result: eyre::Result<Option<Record>>,
    pub cache_hit: bool,
    pub filtered: bool,
}

impl Clone for Cache {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
        }
    }
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
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::Serializer,
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
    pub async fn with_cache<
        'a,
        F: FnOnce(&'a Track) -> Fut,
        Fut: Future<Output = eyre::Result<Option<Record>>>,
    >(
        &self,
        track: &'a Track,
        search: F,
    ) -> CacheResult {
        if let Some(result) = self.map.get(track) {
            // Cache hit; we've searched for this track before, even if we didn't find it.
            return CacheResult {
                result: Ok(result.clone()),
                cache_hit: true,
                filtered: false,
            };
        }

        let record = match search(track).await {
            Ok(record) => record,
            Err(error) => {
                return CacheResult {
                    result: Err(error),
                    cache_hit: false,
                    filtered: false,
                };
            }
        };

        let (record, filtered) = match record {
            None => (None, false),
            Some(rec) => {
                // It's possible we got a search hit, but it's not a real match, and
                // we should filter it out. This is just a guess at a decent heuristic.
                let threshold = 0.7;

                if normalized_damerau_levenshtein(&rec.title, &track.title) < threshold
                    && rec.artists.iter().all(|artist| {
                        normalized_damerau_levenshtein(artist, &track.artist) < threshold
                    })
                {
                    (None, true)
                } else {
                    (Some(rec), false)
                }
            }
        };

        self.map.insert(track.clone(), record.clone());
        CacheResult {
            result: Ok(record),
            cache_hit: false,
            filtered,
        }
    }

    pub async fn get_all<
        'a,
        F: Fn(&'a Track) -> Fut + Clone,
        Fut: Future<Output = eyre::Result<Option<Record>>>,
    >(
        &self,
        tracks: &'a [Track],
        search: F,
    ) -> impl Iterator<Item = Record> {
        let futures = tracks
            .iter()
            .map(|track| self.with_cache(track, search.clone()));
        let results = FuturesOrdered::from_iter(futures).collect::<Vec<_>>().await;
        let cache_hits = results.iter().filter(|r| r.cache_hit).count();
        Span::current().record("cache_hits", cache_hits);
        let filtered = results.iter().filter(|r| r.filtered).count();
        Span::current().record("filtered", filtered);

        results.into_iter().map(|r| r.result).filter_map(|r| {
            if let Err(error) = &r {
                error!(%error, "search failed");
            }
            r.ok().flatten()
        })
    }

    pub fn trim(&self, tracks: &[Track]) {
        let tracks = tracks.iter().collect::<HashSet<_>>();
        self.map.retain(|k, _v| tracks.contains(k));
    }
}

#[cfg(test)]
mod test {
    use std::sync::atomic::{AtomicU32, Ordering::SeqCst};

    use crate::{Record, track::Track};

    use super::Cache;

    #[tokio::test]
    async fn test_cache() {
        let found = Track::new("foo".into(), "fife".into());
        let not_found = Track::new("bar".into(), "bibe".into());
        let new_track = Track::new("car".into(), "cice".into());

        let searches = AtomicU32::new(0);

        let search = async |track: &Track| {
            searches.fetch_add(1, SeqCst);
            if track.artist == "foo" {
                Ok(Some(Record {
                    id: "aaa".into(),
                    title: "N/A".into(),
                    artists: Vec::new(),
                }))
            } else {
                Ok(None)
            }
        };

        let cache = <Cache>::default();
        cache.with_cache(&found, search).await.result.unwrap();
        cache.with_cache(&not_found, search).await.result.unwrap();
        assert_eq!(2, searches.load(SeqCst));

        let str = serde_json::to_string(&cache).unwrap();
        dbg!(&str);
        let cache: Cache = serde_json::from_str(&str).unwrap();

        cache.with_cache(&found, search).await.result.unwrap();
        assert_eq!(2, searches.load(SeqCst));
        cache.with_cache(&not_found, search).await.result.unwrap();
        assert_eq!(2, searches.load(SeqCst));

        cache.with_cache(&new_track, search).await.result.unwrap();
        assert_eq!(3, searches.load(SeqCst));
    }
}
