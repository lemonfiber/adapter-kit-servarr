use lemonfiber_adapter_kit::endpoint::form_encoded;
use lemonfiber_ports::digest::sha256;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::Failure;
use serde::Deserialize;

use super::kind::Shape as _;
use super::wire::QueueRecord;
use super::{Curator, Music};

const GRAB_EVENT: u8 = 1;

const THE_GRABS: &str = "the grab history";

#[derive(Deserialize, Default)]
pub(crate) struct GrabData {
    #[serde(default)]
    pub(crate) guid: Option<String>,
    #[serde(default)]
    pub(crate) indexer: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GrabbedEpisode {
    #[serde(default)]
    pub(crate) season_number: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Grab {
    #[serde(default)]
    pub(crate) source_title: String,
    #[serde(default)]
    pub(crate) episode_id: Option<i64>,
    #[serde(default)]
    pub(crate) episode: Option<GrabbedEpisode>,
    #[serde(default)]
    pub(crate) data: GrabData,
}

impl Grab {
    fn fingerprint(&self) -> Option<String> {
        self.data
            .guid
            .as_deref()
            .filter(|guid| !guid.is_empty())
            .map(sha256)
    }
}

#[derive(Deserialize)]
struct GrabPage {
    #[serde(default)]
    records: Vec<Grab>,
}

impl<M: Music> Curator<M> {
    pub(crate) async fn release_of(&self, record: &QueueRecord) -> Result<Option<String>, Failure> {
        let Some(download) = record.download_id.as_deref().filter(|id| !id.is_empty()) else {
            return Ok(None);
        };
        let page: GrabPage = self
            .servarr
            .read(
                &format!(
                    "/history?page=1&pageSize=1&eventType={GRAB_EVENT}&{}",
                    form_encoded(&[("downloadId", download)])
                ),
                THE_GRABS,
            )
            .await?;
        Ok(page.records.iter().find_map(Grab::fingerprint))
    }

    pub(crate) async fn grabs_of(
        &self,
        kind: Kind,
        id: i64,
        release: &str,
    ) -> Result<Vec<Grab>, Failure> {
        let grabs: Vec<Grab> = self
            .servarr
            .read(
                &format!(
                    "/history/{}?{}={id}&eventType={GRAB_EVENT}{}",
                    kind.library_endpoint(),
                    kind.owner_param(),
                    kind.episode_detail()
                ),
                THE_GRABS,
            )
            .await?;
        Ok(grabs
            .into_iter()
            .filter(|grab| grab.fingerprint().as_deref() == Some(release))
            .collect())
    }
}

#[cfg(test)]
mod tests;
