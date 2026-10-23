use std::collections::BTreeMap;

use async_trait::async_trait;
use lemonfiber_ports::service::stage::Outcome;
use lemonfiber_ports::service::{Failure, Queue, Queued, Queues};
use serde::Deserialize;

use super::wire::{QueueRecord, QueueResource};
use super::{newest_history, Curator, Music};

const QUEUE_PAGE: usize = 200;

const HISTORY_PAGE: usize = 200;

const STUCK: [&str; 2] = ["warning", "error"];

const THE_QUEUE: &str = "the queue";

const QUEUE: &str = "/queue";

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
    episode_id: Option<i64>,
    #[serde(default)]
    movie_id: Option<i64>,
}

impl HistoryRecord {
    const fn item(&self) -> Option<i64> {
        match self.episode_id {
            Some(episode) => Some(episode),
            None => self.movie_id,
        }
    }
}

pub(crate) fn is_stuck(status: &str) -> bool {
    STUCK.iter().any(|stuck| status.eq_ignore_ascii_case(stuck))
}

#[async_trait]
impl<M: Music> Queues for Curator<M> {
    async fn queue(&self) -> Result<Queue, Failure> {
        let queue: QueueResource = self
            .servarr
            .read(&format!("{QUEUE}?pageSize={QUEUE_PAGE}"), THE_QUEUE)
            .await?;
        let grabs = self.grabs().await;
        Ok(Queue {
            total: queue.total_records,
            items: queue
                .records
                .into_iter()
                .map(|record| queued(record, &grabs))
                .collect(),
        })
    }
}

impl<M: Music> Curator<M> {
    pub(crate) async fn queue_pages(&self, query: &str) -> Result<Vec<QueueRecord>, Failure> {
        let mut records = Vec::new();
        let mut page = 1;
        loop {
            let queue: QueueResource = self
                .servarr
                .read(
                    &format!("{QUEUE}?page={page}&pageSize={QUEUE_PAGE}{query}"),
                    THE_QUEUE,
                )
                .await?;
            let on_this_page = queue.records.len();
            records.extend(queue.records);
            if on_this_page < QUEUE_PAGE || page * QUEUE_PAGE >= queue.total_records {
                return Ok(records);
            }
            page += 1;
        }
    }

    async fn grabs(&self) -> BTreeMap<i64, u32> {
        self.servarr
            .read::<HistoryPage>(&newest_history(HISTORY_PAGE), "the history")
            .await
            .map(|page| counted(&page.records))
            .unwrap_or_default()
    }
}

fn counted(records: &[HistoryRecord]) -> BTreeMap<i64, u32> {
    let mut grabs: BTreeMap<i64, u32> = BTreeMap::new();
    let mut settled: BTreeMap<i64, bool> = BTreeMap::new();
    for record in records {
        let Some(item) = record.item() else {
            continue;
        };
        if settled.get(&item).copied().unwrap_or(false) {
            continue;
        }
        match Outcome::of_event(&record.event_type) {
            Some(Outcome::Grabbed) => *grabs.entry(item).or_insert(0) += 1,
            Some(Outcome::Imported) => {
                settled.insert(item, true);
            }
            Some(Outcome::DownloadFailed | Outcome::Removed) | None => {}
        }
    }
    grabs
}

fn queued(record: QueueRecord, grabs: &BTreeMap<i64, u32>) -> Queued {
    let grabbed = record
        .item()
        .and_then(|item| grabs.get(&item).copied())
        .unwrap_or(0);
    Queued {
        title: record.title.unwrap_or_default(),
        status: record.tracked_download_status,
        state: record.tracked_download_state,
        message: record
            .status_messages
            .into_iter()
            .flat_map(|message| message.messages)
            .next()
            .or(record.error_message)
            .filter(|message| !message.trim().is_empty())
            .map(|message| lemonfiber_error::withheld::withheld_text(&message)),
        download_id: record.download_id.filter(|id| !id.is_empty()),
        grabs: grabbed.max(1),
    }
}

#[cfg(test)]
mod tests;
