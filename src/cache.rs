use std::{collections::HashSet, sync::Arc};

use dashmap::DashMap;
use futures::{StreamExt, stream::FuturesOrdered};
use serde::{Deserialize, Serialize};
use tracing::{Span, error};

use crate::{Service, track::Track};

pub struct Cache<S: Service> {
    map: Arc<DashMap<Track, Option<S::Record>>>,
}

impl<S: Service> Default for Cache<S> {
    fn default() -> Self {
        Self {
            map: Default::default(),
        }
    }
}

impl<S: Service> Clone for Cache<S> {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
        }
    }
}

impl<'de, S: Service> Deserialize<'de> for Cache<S> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map = Vec::deserialize(deserializer)?.into_iter().collect();

        Ok(Self { map: Arc::new(map) })
    }
}

impl<S: Service> Serialize for Cache<S> {
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

impl<S: Service> Cache<S> {
    pub async fn with_cache<
        'a,
        F: FnOnce(&'a Track) -> Fut,
        Fut: Future<Output = eyre::Result<Option<S::Record>>>,
    >(
        &self,
        track: &'a Track,
        search: F,
    ) -> (eyre::Result<Option<S::Record>>, bool) {
        if let Some(result) = self.map.get(track) {
            // Cache hit; we've searched for this track before, even if we didn't find it.
            return (Ok(result.clone()), true);
        }

        let record = match search(track).await {
            Ok(record) => record,
            Err(error) => return (Err(error), false),
        };
        self.map.insert(track.clone(), record.clone());
        (Ok(record), false)
    }

    pub async fn get_all<
        'a,
        F: Fn(&'a Track) -> Fut + Clone,
        Fut: Future<Output = eyre::Result<Option<S::Record>>>,
    >(
        &self,
        tracks: &'a [Track],
        search: F,
    ) -> impl Iterator<Item = S::Record> {
        let futures = tracks
            .iter()
            .map(|track| self.with_cache(track, search.clone()));
        let results = FuturesOrdered::from_iter(futures).collect::<Vec<_>>().await;
        let cache_hits = results.iter().filter(|(_, hit)| *hit).count();
        Span::current().record("cache_hits", cache_hits);

        results.into_iter().map(|(r, _)| r).filter_map(|r| {
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

    use crate::{
        tidal::{Record, Tidal},
        track::Track,
    };

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
                Ok(Some(Record::new("aaa".into())))
            } else {
                Ok(None)
            }
        };

        let cache = <Cache<Tidal>>::default();
        cache.with_cache(&found, search).await.0.unwrap();
        cache.with_cache(&not_found, search).await.0.unwrap();
        assert_eq!(2, searches.load(SeqCst));

        let str = serde_json::to_string(&cache).unwrap();
        dbg!(&str);
        let cache: Cache<Tidal> = serde_json::from_str(&str).unwrap();

        cache.with_cache(&found, search).await.0.unwrap();
        assert_eq!(2, searches.load(SeqCst));
        cache.with_cache(&not_found, search).await.0.unwrap();
        assert_eq!(2, searches.load(SeqCst));

        cache.with_cache(&new_track, search).await.0.unwrap();
        assert_eq!(3, searches.load(SeqCst));
    }
}
