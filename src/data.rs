use tracing::Span;

use crate::{Service, cache::Cache, track::Track};

pub struct Data<S: Service> {
    pub cache: Cache<S>,
    pub client: reqwest::Client,
    pub settings: S::Settings,
    pub tracks: Vec<Track>,
}

impl<S: Service> Data<S> {
    pub fn new(
        cache: &Cache<S>,
        client: &reqwest::Client,
        settings: S::Settings,
        tracks: &[Track],
    ) -> Self {
        Self {
            cache: cache.clone(),
            settings,
            client: client.clone(),
            tracks: tracks.to_owned(),
        }
    }

    pub async fn search_all<
        'a,
        F: Fn(&'a Track) -> Fut + Clone,
        Fut: Future<Output = eyre::Result<Option<S::Record>>>,
    >(
        &'a self,
        search: F,
    ) -> Vec<S::Record> {
        let records = self
            .cache
            .get_all(&self.tracks, search)
            .await
            .collect::<Vec<_>>();

        Span::current().record("found", records.len());
        records
    }
}
