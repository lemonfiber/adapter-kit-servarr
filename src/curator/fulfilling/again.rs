use lemonfiber_ports::digest::sha256;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Failure, Grabbed};
use serde::Deserialize;
use serde_json::json;

use super::super::filed::Filed;
use super::super::grabs::Grab;
use super::super::kind::Shape as _;
use super::super::queue::QUEUE;
use super::super::wire::Page;
use super::super::{Curator, Music, RELEASE, THE_RELEASE_SEARCH};

const BLOCKLIST: &str = "/blocklist";

const BLOCKLIST_PAGE: usize = 1000;

const THE_BLOCKLIST: &str = "the blocklist";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Blocked {
    id: i64,
    #[serde(default)]
    source_title: String,
    #[serde(default)]
    indexer: Option<String>,
    #[serde(default)]
    album_ids: Vec<i64>,
}

impl Blocked {
    fn is_of(&self, grab: &Grab) -> bool {
        self.source_title == grab.source_title && self.indexer == grab.data.indexer
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Offered {
    #[serde(default)]
    guid: String,
    #[serde(default)]
    indexer_id: i64,
}

fn searched_by(filed: Filed, id: i64, grabs: &[Grab]) -> Option<String> {
    let kind = match filed {
        Filed::Video(Kind::Movies) | Filed::Album => {
            return Some(format!("{RELEASE}?{}={id}", filed.owner_param()));
        }
        Filed::Video(kind) => kind,
    };
    match grabs {
        [] => None,
        [one] => one
            .episode_id
            .map(|episode| format!("{RELEASE}?{}={episode}", kind.release_id_param())),
        [first, ..] => first.episode.as_ref().map(|episode| {
            format!(
                "{RELEASE}?{}={id}&seasonNumber={}",
                kind.owner_param(),
                episode.season_number
            )
        }),
    }
}

impl<M: Music> Curator<M> {
    pub(super) async fn unqueued(&self, id: i64, blocklist: bool) -> Result<String, Failure> {
        let record = self
            .queue_pages("")
            .await?
            .into_iter()
            .find(|record| record.id == Some(id))
            .ok_or_else(|| {
                self.servarr
                    .endpoint()
                    .refused(&format!("it holds no queue entry {id}"))
            })?;
        let release = self.release_of(&record).await?.ok_or_else(|| {
            self.servarr
                .endpoint()
                .refused(&format!("its history holds no grab of queue entry {id}"))
        })?;
        self.servarr
            .done(
                Method::Delete,
                &format!(
                    "{QUEUE}/{id}?removeFromClient=true&blocklist={blocklist}&skipRedownload=true"
                ),
                None,
            )
            .await?;
        Ok(release)
    }

    pub(super) async fn grabbed_again(
        &self,
        filed: Filed,
        id: i64,
        artist: i64,
        release: &str,
    ) -> Result<Grabbed, Failure> {
        let grabs = self.grabs_of(filed, id, artist, release).await?;
        self.lifted(filed, id, &grabs).await?;
        let Some(search) = searched_by(filed, id, &grabs) else {
            return Ok(Grabbed::Gone);
        };
        let offered: Vec<Offered> = self.servarr.read(&search, THE_RELEASE_SEARCH).await?;
        let Some(found) = offered
            .into_iter()
            .find(|offer| sha256(&offer.guid) == release)
        else {
            return Ok(Grabbed::Gone);
        };
        self.servarr
            .done(
                Method::Post,
                RELEASE,
                Some(
                    json!({
                        "guid": found.guid,
                        "indexerId": found.indexer_id,
                        filed.owner_param(): id,
                    })
                    .to_string(),
                ),
            )
            .await?;
        Ok(Grabbed::Grabbed)
    }

    async fn lifted(&self, filed: Filed, id: i64, grabs: &[Grab]) -> Result<(), Failure> {
        if grabs.is_empty() {
            return Ok(());
        }
        let blocked = match filed.blocklist_filter() {
            Some(filter) => {
                self.servarr
                    .read::<Page<Blocked>>(
                        &format!("{BLOCKLIST}?page=1&pageSize={BLOCKLIST_PAGE}&{filter}={id}"),
                        THE_BLOCKLIST,
                    )
                    .await?
                    .records
            }
            None => self
                .paged::<Blocked>(BLOCKLIST, BLOCKLIST_PAGE, "", THE_BLOCKLIST)
                .await?
                .into_iter()
                .filter(|entry| entry.album_ids.contains(&id))
                .collect(),
        };
        for entry in blocked
            .iter()
            .filter(|entry| grabs.iter().any(|grab| entry.is_of(grab)))
        {
            self.servarr
                .done(Method::Delete, &format!("{BLOCKLIST}/{}", entry.id), None)
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
