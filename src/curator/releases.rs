use async_trait::async_trait;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Failure, QualityReleases, ReleaseProbe};
use serde::Deserialize;

use super::kind::Shape as _;
use super::{Curator, Music, RELEASE, THE_RELEASE_SEARCH, THE_WANTED_LIST};

const FIRST_WANTED: &str = "/wanted/missing?page=1&pageSize=1";

#[derive(Deserialize)]
struct Wanted {
    #[serde(default)]
    records: Vec<WantedRecord>,
}

#[derive(Deserialize)]
struct WantedRecord {
    id: i64,
}

#[derive(Deserialize)]
struct ReleaseResource {
    #[serde(default)]
    rejections: Vec<String>,
}

#[async_trait]
impl<M: Music> QualityReleases for Curator<M> {
    async fn probe_releases(&self, kind: Kind) -> Result<ReleaseProbe, Failure> {
        let kind = self.filing(kind)?;
        let wanted: Wanted = self.servarr.read(FIRST_WANTED, THE_WANTED_LIST).await?;
        let Some(first) = wanted.records.first() else {
            return Ok(ReleaseProbe::NothingWanted);
        };
        let releases: Vec<ReleaseResource> = self
            .servarr
            .read(
                &format!("{RELEASE}?{}={}", kind.release_id_param(), first.id),
                THE_RELEASE_SEARCH,
            )
            .await?;
        Ok(if releases.is_empty() {
            ReleaseProbe::NoneFound
        } else if releases.iter().any(|release| release.rejections.is_empty()) {
            ReleaseProbe::Matching
        } else {
            ReleaseProbe::NoneMatch
        })
    }
}

#[cfg(test)]
mod tests;
