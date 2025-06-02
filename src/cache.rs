use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;
use futures::{StreamExt, stream::FuturesOrdered};
use serde::{Deserialize, Serialize};
use strsim::normalized_damerau_levenshtein;
use tracing::{Span, error};

use crate::{Record, track::Track};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedRecord {
    record: Record,
    rejected: bool,
}

/// Attempt to make a canonical representation of the artist.
fn artist_str(artist: &str) -> String {
    artist.to_lowercase()
}

/// Attempt to make a canonical representation of the title.
fn title_str(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        // A lot of tracks have parentheticals which don't match between the query and result.
        .take_while(|ch| *ch != '(')
        .collect::<String>()
}

impl CachedRecord {
    fn new(record: Record, track: &Track) -> Self {
        // It's possible we got a search hit, but it's not a real match, and
        // we should filter it out.
        //
        // This is just a guess at a decent heuristic.
        let threshold = 0.7;

        let rejected =
            normalized_damerau_levenshtein(&title_str(&track.title), &title_str(&record.title))
                < threshold
                || record.artists.iter().all(|artist| {
                    normalized_damerau_levenshtein(&artist_str(&track.artist), &artist_str(artist))
                        < threshold
                });

        Self { record, rejected }
    }
}

#[derive(Default)]
pub struct Cache {
    map: Arc<DashMap<Track, Option<CachedRecord>>>,
}

pub struct CacheResult {
    record: eyre::Result<Option<CachedRecord>>,
    cache_hit: bool,
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
        if let Some(record) = self.map.get(track) {
            // Cache hit; we've searched for this track before, even if we didn't find it.
            return CacheResult {
                record: Ok(record.clone()),
                cache_hit: true,
            };
        }

        let record = match search(track).await {
            Ok(record) => record.map(|r| CachedRecord::new(r, track)),
            Err(error) => {
                return CacheResult {
                    record: Err(error),
                    cache_hit: false,
                };
            }
        };

        self.map.insert(track.clone(), record.clone());

        CacheResult {
            record: Ok(record),
            cache_hit: false,
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
        // let rejected = results.iter().filter(|r| r.record).count();
        // Span::current().record("filtered", filtered);

        let records = results
            .into_iter()
            .map(|r| r.record)
            .filter_map(|r| {
                if let Err(error) = &r {
                    error!(%error, "search failed");
                }
                r.ok().flatten()
            })
            .collect::<Vec<_>>();

        let rejected = records.iter().filter(|r| r.rejected).count();
        Span::current().record("rejected", rejected);

        records
            .into_iter()
            .filter_map(|r| if r.rejected { None } else { Some(r.record) })
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
        cache.with_cache(&found, search).await.record.unwrap();
        cache.with_cache(&not_found, search).await.record.unwrap();
        assert_eq!(2, searches.load(SeqCst));

        let str = serde_json::to_string(&cache).unwrap();
        dbg!(&str);
        let cache: Cache = serde_json::from_str(&str).unwrap();

        cache.with_cache(&found, search).await.record.unwrap();
        assert_eq!(2, searches.load(SeqCst));
        cache.with_cache(&not_found, search).await.record.unwrap();
        assert_eq!(2, searches.load(SeqCst));

        cache.with_cache(&new_track, search).await.record.unwrap();
        assert_eq!(3, searches.load(SeqCst));
    }
}
