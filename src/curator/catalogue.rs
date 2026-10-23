use async_trait::async_trait;
use lemonfiber_adapter_kit::endpoint::form_encoded;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{AddPlan, Added, Catalogue, CatalogueEntry, Failure};
use serde::Deserialize;

use super::kind::Shape as _;
use super::wire::ProfileResource;
use super::{Curator, Music, QUALITY_PROFILES, ROOT_FOLDERS};

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
                "the catalogue",
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
            self.servarr.read(ROOT_FOLDERS, "the root folders").await?;
        let profiles: Vec<ProfileResource> = self
            .servarr
            .read(QUALITY_PROFILES, "the quality profiles")
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
                self.servarr.unsupported(&format!(
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
        let (only, value) = kind.only_field();
        let fields: serde_json::Map<String, serde_json::Value> = [
            ("title", serde_json::json!(entry.title)),
            ("qualityProfileId", serde_json::json!(plan.quality_profile)),
            ("rootFolderPath", serde_json::json!(plan.root_folder)),
            ("monitored", serde_json::json!(true)),
            (
                "addOptions",
                serde_json::json!({ kind.search_option(): true }),
            ),
            (kind.reference_field(), serde_json::json!(entry.reference)),
            (only, value),
        ]
        .into_iter()
        .map(|(at, value)| (at.to_owned(), value))
        .collect();
        let response = self
            .servarr
            .ask(
                Method::Post,
                &format!("/{}", kind.library_endpoint()),
                Some(serde_json::Value::Object(fields).to_string()),
            )
            .await?;
        let added: LookupResult = self
            .servarr
            .endpoint()
            .decode(&response, "the service did not say what it took on")?;
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

#[cfg(test)]
mod tests;
