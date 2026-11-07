use async_trait::async_trait;
use lemonfiber_adapter_kit::endpoint::form_encoded;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{AddPlan, Added, Catalogue, CatalogueEntry, Failure};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::kind::Shape as _;
use super::wire::ProfileResource;
use super::{
    Curator, Music, MONITORED, QUALITY_PROFILES, ROOT_FOLDERS, THE_CATALOGUE, THE_QUALITY_PROFILES,
    THE_ROOT_FOLDERS,
};

const INDEXERS: &str = "/indexer";

const TERM: &str = "term";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LookupResult {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    year: Option<u32>,
    #[serde(default)]
    tvdb_id: Option<i64>,
    #[serde(default)]
    tmdb_id: Option<i64>,
}

impl LookupResult {
    fn entry(self, kind: Kind) -> CatalogueEntry {
        CatalogueEntry {
            title: self.title,
            year: self.year,
            reference: match kind {
                Kind::Tv => self.tvdb_id,
                Kind::Movies => self.tmdb_id,
            }
            .unwrap_or_default(),
            held_as: (self.id != 0).then_some(self.id),
        }
    }
}

#[derive(Deserialize)]
struct RootFolderResource {
    #[serde(default)]
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexerResource {
    #[serde(default)]
    enable_automatic_search: bool,
    #[serde(default)]
    enable_interactive_search: bool,
}

#[async_trait]
impl<M: Music> Catalogue for Curator<M> {
    async fn lookup(&self, kind: Kind, term: &str) -> Result<Vec<CatalogueEntry>, Failure> {
        let kind = self.filing(kind)?;
        let query = form_encoded(&[(TERM, term)]);
        let results: Vec<LookupResult> = self
            .servarr
            .read(
                &format!("/{}/lookup?{query}", kind.library_endpoint()),
                THE_CATALOGUE,
            )
            .await?;
        Ok(results
            .into_iter()
            .map(|result| result.entry(kind))
            .collect())
    }

    async fn add_plan(&self, kind: Kind) -> Result<AddPlan, Failure> {
        let kind = self.filing(kind)?;
        let folders: Vec<RootFolderResource> =
            self.servarr.read(ROOT_FOLDERS, THE_ROOT_FOLDERS).await?;
        let profiles: Vec<ProfileResource> = self
            .servarr
            .read(QUALITY_PROFILES, THE_QUALITY_PROFILES)
            .await?;
        let root_folder = folders.into_iter().map(|folder| folder.path).next();
        let quality_profile = profiles.into_iter().map(|profile| profile.id).next();
        root_folder
            .zip(quality_profile)
            .map(|(root_folder, quality_profile)| AddPlan {
                root_folder,
                quality_profile,
            })
            .ok_or_else(|| {
                self.servarr.endpoint().refused(&format!(
                    "{} has no root folder or no quality profile configured yet",
                    kind.noun()
                ))
            })
    }

    async fn add(
        &self,
        kind: Kind,
        entry: &CatalogueEntry,
        plan: &AddPlan,
    ) -> Result<Added, Failure> {
        let kind = self.filing(kind)?;
        let added: LookupResult = self
            .post_addition(
                kind,
                addition(kind, &entry.title, entry.reference, plan, true),
            )
            .await?;
        Ok(Added {
            id: added.id,
            title: added.title,
        })
    }

    async fn indexer_count(&self) -> Result<usize, Failure> {
        let indexers: Vec<IndexerResource> = self.servarr.read(INDEXERS, "the indexers").await?;
        Ok(indexers
            .into_iter()
            .filter(|indexer| indexer.enable_automatic_search || indexer.enable_interactive_search)
            .count())
    }
}

impl<M: Music> Curator<M> {
    pub(super) async fn post_addition<T: serde::de::DeserializeOwned>(
        &self,
        kind: Kind,
        fields: Map<String, Value>,
    ) -> Result<T, Failure> {
        let response = self
            .servarr
            .ask(
                Method::Post,
                &format!("/{}", kind.library_endpoint()),
                Some(Value::Object(fields).to_string()),
            )
            .await?;
        self.servarr
            .endpoint()
            .decode(&response, "the service did not say what it took on")
    }
}

pub(super) fn addition(
    kind: Kind,
    title: &str,
    reference: i64,
    plan: &AddPlan,
    search: bool,
) -> Map<String, Value> {
    let (only, value) = kind.only_field();
    [
        ("title", json!(title)),
        ("qualityProfileId", json!(plan.quality_profile)),
        ("rootFolderPath", json!(plan.root_folder)),
        (MONITORED, json!(true)),
        ("addOptions", json!({ kind.search_option(): search })),
        (kind.reference_field(), json!(reference)),
        (only, value),
    ]
    .into_iter()
    .map(|(at, value)| (at.to_owned(), value))
    .collect()
}

#[cfg(test)]
mod tests;
