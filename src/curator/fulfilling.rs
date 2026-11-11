use async_trait::async_trait;
use lemonfiber_adapter_kit::endpoint::form_encoded;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::{Kind, TitleKind};
use lemonfiber_ports::service::{
    AddPlan, CuratedTitle, Downloading, Failure, Fulfilling, Grabbed, TitleRequest,
};
use serde::Deserialize as _;
use serde_json::{json, Map, Value};

use super::catalogue::addition;
use super::filed::Filed;
use super::kind::Shape as _;
use super::wire::{FolderResource, ProfileResource};
use super::{
    Curator, Music, ARTIST_ID, COMMAND, MONITORED, QUALITY_PROFILES, ROOT_FOLDERS, THE_CATALOGUE,
    THE_EPISODES, THE_LIBRARY, THE_QUALITY_PROFILES, THE_ROOT_FOLDERS,
};

mod again;
mod downloads;
mod titled;

use titled::TitleResource;

const ID: &str = "id";

const SEASONS: &str = "seasons";

const SEASON_NUMBER: &str = "seasonNumber";

const THE_TITLE: &str = "the title";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartResource {
    id: i64,
    #[serde(default)]
    season_number: u32,
    #[serde(default)]
    monitored: bool,
}

#[async_trait]
impl<M: Music> Fulfilling for Curator<M> {
    async fn catalogued(
        &self,
        kind: Kind,
        reference: i64,
    ) -> Result<Option<CuratedTitle>, Failure> {
        let kind = self.filing(kind)?;
        let found = match self.held_by_reference(kind, reference).await? {
            Some(held) => Some(
                TitleResource::deserialize(Value::Object(held)).map_err(|_| {
                    self.servarr
                        .endpoint()
                        .refused("the title it holds could not be read")
                })?,
            ),
            None => self.looked_up(kind, reference).await?,
        };
        Ok(found.map(|title| title.curated(kind.into())))
    }

    async fn titles(&self, kind: TitleKind) -> Result<Vec<CuratedTitle>, Failure> {
        let kind = self.filed(kind)?;
        let held: Vec<TitleResource> = self.servarr.read(&library(kind), THE_LIBRARY).await?;
        Ok(held.into_iter().map(|title| title.curated(kind)).collect())
    }

    async fn held_title(&self, kind: Kind, id: i64) -> Result<Option<CuratedTitle>, Failure> {
        let kind = self.filing(kind)?;
        let held: Option<TitleResource> = self
            .servarr
            .find(&titled(kind.into(), id), THE_TITLE)
            .await?;
        Ok(held.map(|title| title.curated(kind.into())))
    }

    async fn request(&self, kind: Kind, request: &TitleRequest) -> Result<CuratedTitle, Failure> {
        let kind = self.filing(kind)?;
        let title = match self.held_by_reference(kind, request.reference).await? {
            Some(held) => self.monitored(kind, held, request).await?,
            None => self.added(kind, request).await?,
        };
        Ok(title.curated(kind.into()))
    }

    async fn search(&self, kind: TitleKind, id: i64) -> Result<(), Failure> {
        let kind = self.filed(kind)?;
        self.holding_title(kind, id).await?;
        self.searched(kind, id).await
    }

    async fn remove(&self, kind: Kind, id: i64) -> Result<(), Failure> {
        let kind = self.filing(kind)?;
        self.holding_title(kind.into(), id).await?;
        self.servarr
            .done(
                Method::Delete,
                &format!(
                    "{}?deleteFiles=true&{}=false",
                    titled(kind.into(), id),
                    kind.exclusion_param()
                ),
                None,
            )
            .await
    }

    async fn downloading(&self, kind: TitleKind) -> Result<Vec<Downloading>, Failure> {
        let kind = self.filed(kind)?;
        Ok(self
            .queue_pages(kind.episode_detail())
            .await?
            .into_iter()
            .filter_map(|record| downloads::downloading(kind, record))
            .collect())
    }

    async fn unqueue(&self, kind: TitleKind, id: i64, blocklist: bool) -> Result<String, Failure> {
        self.filed(kind)?;
        self.unqueued(id, blocklist).await
    }

    async fn grab(&self, kind: TitleKind, id: i64, release: &str) -> Result<Grabbed, Failure> {
        let kind = self.filed(kind)?;
        let held = self.holding_title(kind, id).await?;
        let artist = held
            .get(ARTIST_ID)
            .and_then(Value::as_i64)
            .unwrap_or_default();
        self.grabbed_again(kind, id, artist, release).await
    }
}

impl<M: Music> Curator<M> {
    async fn held_by_reference(
        &self,
        kind: Kind,
        reference: i64,
    ) -> Result<Option<Map<String, Value>>, Failure> {
        let field = kind.reference_field();
        let held: Vec<Map<String, Value>> = self
            .servarr
            .read(
                &format!("{}?{field}={reference}", library(kind.into())),
                THE_LIBRARY,
            )
            .await?;
        Ok(held
            .into_iter()
            .find(|title| title.get(field).and_then(Value::as_i64) == Some(reference)))
    }

    async fn looked_up(
        &self,
        kind: Kind,
        reference: i64,
    ) -> Result<Option<TitleResource>, Failure> {
        let term = format!("{}:{reference}", kind.catalogue());
        let found: Vec<TitleResource> = self
            .servarr
            .read(
                &format!(
                    "{}/lookup?{}",
                    library(kind.into()),
                    form_encoded(&[("term", &term)])
                ),
                THE_CATALOGUE,
            )
            .await?;
        Ok(found
            .into_iter()
            .find(|title| title.reference(kind.into()) == Some(reference)))
    }

    async fn holding_title(&self, filed: Filed, id: i64) -> Result<Value, Failure> {
        let held: Option<Value> = self.servarr.find(&titled(filed, id), THE_TITLE).await?;
        held.ok_or_else(|| {
            self.servarr
                .endpoint()
                .refused(&format!("it holds no {} as {id}", filed.noun()))
        })
    }

    async fn searched(&self, filed: Filed, id: i64) -> Result<(), Failure> {
        self.servarr
            .done(
                Method::Post,
                COMMAND,
                Some(filed.title_search(id).to_string()),
            )
            .await
    }

    async fn added(&self, kind: Kind, request: &TitleRequest) -> Result<TitleResource, Failure> {
        let profiles: Vec<ProfileResource> = self
            .servarr
            .read(QUALITY_PROFILES, THE_QUALITY_PROFILES)
            .await?;
        if !profiles
            .iter()
            .any(|profile| profile.id == request.quality_profile)
        {
            return Err(self.servarr.endpoint().refused(&format!(
                "it holds no quality profile {}",
                request.quality_profile
            )));
        }
        let folders: Vec<FolderResource> =
            self.servarr.read(ROOT_FOLDERS, THE_ROOT_FOLDERS).await?;
        let asked = request.root_folder.trim_end_matches('/');
        let folder = folders
            .into_iter()
            .map(|folder| folder.path)
            .find(|path| path.trim_end_matches('/') == asked)
            .ok_or_else(|| {
                self.servarr
                    .endpoint()
                    .refused(&format!("it holds no root folder {}", request.root_folder))
            })?;
        let found = self
            .looked_up(kind, request.reference)
            .await?
            .ok_or_else(|| {
                self.servarr.endpoint().refused(&format!(
                    "its catalogue holds no {} as {}",
                    kind.noun(),
                    request.reference
                ))
            })?;
        let plan = AddPlan {
            root_folder: folder,
            quality_profile: request.quality_profile,
        };
        let mut fields = addition(kind, &found.title, request.reference, &plan, request.search);
        if kind == Kind::Tv {
            fields.extend(series_fields(&found, request));
        }
        self.post_addition(kind, fields).await
    }

    async fn monitored(
        &self,
        kind: Kind,
        mut held: Map<String, Value>,
        request: &TitleRequest,
    ) -> Result<TitleResource, Failure> {
        let id = held.get(ID).and_then(Value::as_i64).unwrap_or_default();
        held.insert(MONITORED.to_owned(), Value::Bool(true));
        held.get_mut(SEASONS)
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object_mut)
            .filter(|season| asked_for(season, &request.seasons))
            .for_each(|season| {
                season.insert(MONITORED.to_owned(), Value::Bool(true));
            });
        let response = self
            .servarr
            .ask(
                Method::Put,
                &titled(kind.into(), id),
                Some(Value::Object(held).to_string()),
            )
            .await?;
        let title: TitleResource = self
            .servarr
            .endpoint()
            .decode(&response, "the service did not say what it monitors")?;
        if let Some(parts) = kind.parts().filter(|_| !request.seasons.is_empty()) {
            self.parts_monitored(parts, id, &request.seasons).await?;
        }
        if request.search {
            self.searched(kind.into(), id).await?;
        }
        Ok(title)
    }

    async fn parts_monitored(
        &self,
        (endpoint, filter): (&str, &str),
        id: i64,
        seasons: &[u32],
    ) -> Result<(), Failure> {
        let parts: Vec<PartResource> = self
            .servarr
            .read(&format!("/{endpoint}?{filter}={id}"), THE_EPISODES)
            .await?;
        let unmonitored: Vec<i64> = parts
            .into_iter()
            .filter(|part| !part.monitored && seasons.contains(&part.season_number))
            .map(|part| part.id)
            .collect();
        if unmonitored.is_empty() {
            return Ok(());
        }
        self.servarr
            .done(
                Method::Put,
                &format!("/{endpoint}/monitor"),
                Some(json!({ "episodeIds": unmonitored, MONITORED: true }).to_string()),
            )
            .await
    }
}

fn library(filed: Filed) -> String {
    format!("/{}", filed.library_endpoint())
}

fn titled(filed: Filed, id: i64) -> String {
    format!("/{}/{id}", filed.library_endpoint())
}

fn asked_for(season: &Map<String, Value>, seasons: &[u32]) -> bool {
    season
        .get(SEASON_NUMBER)
        .and_then(Value::as_u64)
        .and_then(|number| u32::try_from(number).ok())
        .is_some_and(|number| seasons.contains(&number))
}

fn series_fields(found: &TitleResource, request: &TitleRequest) -> [(String, Value); 3] {
    let seasons = found
        .season_numbers()
        .map(
            |number| json!({ SEASON_NUMBER: number, MONITORED: request.seasons.contains(&number) }),
        )
        .collect();
    [
        (SEASONS.to_owned(), Value::Array(seasons)),
        (
            "seriesType".to_owned(),
            json!(if request.anime { "anime" } else { "standard" }),
        ),
        (
            "monitorNewItems".to_owned(),
            json!(if request.later_seasons { "all" } else { "none" }),
        ),
    ]
}

#[cfg(test)]
mod tests;
