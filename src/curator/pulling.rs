//! `indexer.pull` as Sonarr, Radarr and Lidarr answer it: an aggregator read as one Newznab
//! indexer of the curator's own, made from the curator's own Newznab schema.

use async_trait::async_trait;
use lemonfiber_manifest::ApiKind;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{AggregatorAt, Failure, KnownAggregator, PullsIndexers};
use serde_json::{Map, Value};

use super::clients::{set_field, FIELDS, NAME, VALUE};
use super::{Curator, Music, INDEXERS, THE_INDEXERS, UNTESTED};

const ID: &str = "id";

const SCHEMA: &str = "/indexer/schema";

const NEWZNAB: &str = "Newznab";

const IMPLEMENTATION: &str = "implementation";

const PRESETS: &str = "presets";

const BASE_URL: &str = "baseUrl";

const API_KEY: &str = "apiKey";

const SWITCHES: [&str; 3] = [
    "enableRss",
    "enableAutomaticSearch",
    "enableInteractiveSearch",
];

const PULLS_FROM: &str = "it pulls its indexers from NZBHydra2 alone, read as a Newznab indexer";

const NO_NEWZNAB: &str = "it offers no Newznab indexer to read NZBHydra2 as";

const NO_SETTINGS: &str = "the Newznab indexer has no settings to write";

const UNNAMED: &str = "the service did not say which indexer it made";

const NO_PULL: &str = "it reads NZBHydra2 through one Newznab indexer at every search, so there is nothing to read again";

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

type Entry = Map<String, Value>;

fn field<'a>(entry: &'a Entry, name: &str) -> &'a str {
    entry
        .get(FIELDS)
        .and_then(Value::as_array)
        .and_then(|fields| fields.iter().find(|one| text(one, NAME) == name))
        .map(|one| text(one, VALUE))
        .unwrap_or_default()
}

fn newznab(entry: &Entry) -> bool {
    entry.get(IMPLEMENTATION).and_then(Value::as_str) == Some(NEWZNAB)
}

fn switched_on(entry: &Entry) -> bool {
    SWITCHES
        .iter()
        .any(|switch| entry.get(*switch).and_then(Value::as_bool) == Some(true))
}

fn at_url(entry: &Entry, url: &str) -> bool {
    field(entry, BASE_URL).trim_end_matches('/') == url.trim_end_matches('/')
}

fn known(entry: &Entry) -> Option<KnownAggregator> {
    let url = field(entry, BASE_URL);
    let id = entry.get(ID).and_then(Value::as_i64)?;
    (!url.is_empty()).then(|| KnownAggregator {
        id: id.to_string(),
        url: url.to_owned(),
        keyed: !field(entry, API_KEY).is_empty(),
    })
}

fn switched(entry: &mut Entry, on: bool) {
    for switch in SWITCHES {
        entry.insert(switch.to_owned(), Value::Bool(on));
    }
}

impl<M: Music> Curator<M> {
    fn settings<'a>(&self, entry: &'a mut Entry) -> Result<&'a mut Vec<Value>, Failure> {
        entry
            .get_mut(FIELDS)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| self.servarr.endpoint().refused(NO_SETTINGS))
    }

    async fn held_indexers(&self) -> Result<Vec<Entry>, Failure> {
        self.servarr.read(INDEXERS, THE_INDEXERS).await
    }

    async fn made(&self, aggregator: &AggregatorAt) -> Result<Entry, Failure> {
        let schema: Vec<Entry> = self.servarr.read(SCHEMA, "the indexers it offers").await?;
        let mut entry = schema
            .into_iter()
            .find(newznab)
            .ok_or_else(|| self.servarr.unsupported(NO_NEWZNAB))?;
        entry.remove(PRESETS);
        entry.insert(NAME.to_owned(), Value::from(aggregator.name.as_str()));
        switched(&mut entry, false);
        let settings = self.settings(&mut entry)?;
        set_field(settings, BASE_URL, Some(&aggregator.url));
        set_field(settings, API_KEY, Some(&aggregator.key));
        let response = self
            .servarr
            .ask(
                Method::Post,
                INDEXERS,
                Some(Value::Object(entry).to_string()),
            )
            .await?;
        self.servarr.endpoint().decode(&response, UNNAMED)
    }
}

#[async_trait]
impl<M: Music> PullsIndexers for Curator<M> {
    async fn aggregators(&self) -> Result<Vec<KnownAggregator>, Failure> {
        Ok(self
            .held_indexers()
            .await?
            .iter()
            .filter(|entry| newznab(entry) && switched_on(entry))
            .filter_map(known)
            .collect())
    }

    async fn add_aggregator(&self, aggregator: &AggregatorAt) -> Result<(), Failure> {
        if aggregator.protocol.0 != ApiKind::Nzbhydra2.name() {
            return Err(self.servarr.unsupported(PULLS_FROM));
        }
        let held = self
            .held_indexers()
            .await?
            .into_iter()
            .find(|entry| newznab(entry) && at_url(entry, &aggregator.url));
        let mut entry = match held {
            Some(entry) => entry,
            None => self.made(aggregator).await?,
        };
        let id = entry
            .get(ID)
            .and_then(Value::as_i64)
            .ok_or_else(|| self.servarr.endpoint().refused(UNNAMED))?;
        switched(&mut entry, true);
        set_field(self.settings(&mut entry)?, API_KEY, Some(&aggregator.key));
        self.servarr
            .done(
                Method::Put,
                &format!("{INDEXERS}/{id}{UNTESTED}"),
                Some(Value::Object(entry).to_string()),
            )
            .await
    }

    async fn pull(&self) -> Result<(), Failure> {
        Err(self.servarr.unsupported(NO_PULL))
    }
}

#[cfg(test)]
mod tests;
