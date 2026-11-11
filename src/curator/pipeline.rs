use std::collections::BTreeSet;

use async_trait::async_trait;
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::stage::{History, Outcome, Stage, HISTORY_HORIZON};
use lemonfiber_ports::service::{
    Failure, FoundItem, ItemPart, Pipeline, QueueItem, StuckItem, TraceEvent,
};
use serde::Deserialize;

use super::queue::is_stuck;
use super::{newest_history, Curator, Music, THE_EPISODES, THE_HISTORY, THE_LIBRARY};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryItem {
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    year: Option<u16>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    monitored: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartResource {
    id: i64,
    #[serde(default)]
    season_number: u32,
    #[serde(default)]
    episode_number: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    monitored: bool,
    #[serde(default)]
    has_file: bool,
}

#[derive(Deserialize)]
struct HistoryPage {
    #[serde(default)]
    records: Vec<HistoryRecord>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryRecord {
    #[serde(default)]
    event_type: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    episode_id: Option<i64>,
}

fn staged(state: &str) -> Stage {
    Stage::of_queue_state(state).unwrap_or(Stage::Downloading)
}

#[async_trait]
impl<M: Music> Pipeline for Curator<M> {
    async fn library(&self, kind: TitleKind) -> Result<Vec<FoundItem>, Failure> {
        let kind = self.filed(kind)?;
        let items: Vec<LibraryItem> = self
            .servarr
            .read(&format!("/{}", kind.library_endpoint()), THE_LIBRARY)
            .await?;
        Ok(items
            .into_iter()
            .map(|item| FoundItem {
                id: item.id,
                title: item.title,
                year: kind
                    .year(item.year, item.release_date.as_deref())
                    .filter(|year| *year > 0),
                monitored: item.monitored,
            })
            .collect())
    }

    async fn find_items(&self, kind: TitleKind, term: &str) -> Result<Vec<FoundItem>, Failure> {
        let needle = term.to_lowercase();
        Ok(self
            .library(kind)
            .await?
            .into_iter()
            .filter(|item| item.title.to_lowercase().contains(&needle))
            .collect())
    }

    async fn item_history(&self, kind: TitleKind, id: i64) -> Result<Vec<TraceEvent>, Failure> {
        let kind = self.filed(kind)?;
        let page: HistoryPage = self
            .servarr
            .read(
                &format!(
                    "{}&{}={id}",
                    newest_history(HISTORY_HORIZON),
                    kind.history_filter()
                ),
                THE_HISTORY,
            )
            .await?;
        Ok(page
            .records
            .into_iter()
            .filter_map(|record| {
                Outcome::of_event(History::Servarr, &record.event_type).map(|outcome| TraceEvent {
                    outcome,
                    at: record.date,
                    part: record.episode_id,
                })
            })
            .collect())
    }

    async fn item_queue(&self, kind: TitleKind, id: i64) -> Result<Vec<QueueItem>, Failure> {
        let kind = self.filed(kind)?;
        Ok(self
            .queue_pages("")
            .await?
            .iter()
            .filter(|record| record.is_for(kind, id))
            .map(|record| QueueItem {
                part: record.episode_id,
                stage: staged(&record.tracked_download_state),
                stuck: is_stuck(&record.tracked_download_status),
            })
            .collect())
    }

    async fn item_parts(
        &self,
        kind: TitleKind,
        id: i64,
        season: Option<u32>,
    ) -> Result<Vec<ItemPart>, Failure> {
        let kind = self.filed(kind)?;
        let Some((endpoint, filter)) = kind.parts() else {
            return Ok(Vec::new());
        };
        let season = season.map_or_else(String::new, |number| format!("&seasonNumber={number}"));
        let parts: Vec<PartResource> = self
            .servarr
            .read(&format!("/{endpoint}?{filter}={id}{season}"), THE_EPISODES)
            .await?;
        Ok(parts
            .into_iter()
            .map(|part| ItemPart {
                id: part.id,
                season: part.season_number,
                number: part.episode_number,
                title: part.title,
                monitored: part.monitored,
                has_file: part.has_file,
            })
            .collect())
    }

    async fn stuck_items(&self, kind: TitleKind) -> Result<Vec<StuckItem>, Failure> {
        let kind = self.filed(kind)?;
        let records = self.queue_pages(kind.with_items()).await?;
        let mut seen = BTreeSet::new();
        let mut stuck = Vec::new();
        for (title, record) in records
            .iter()
            .filter(|record| is_stuck(&record.tracked_download_status))
            .filter_map(|record| record.item_title(kind).map(|title| (title, record)))
            .filter(|(title, _)| seen.insert(title.clone()))
        {
            stuck.push(StuckItem {
                title,
                stage: staged(&record.tracked_download_state),
                queued: record.id,
                title_id: record.owner(kind),
                release: self.release_of(record).await?,
            });
        }
        Ok(stuck)
    }
}

#[cfg(test)]
mod tests;
