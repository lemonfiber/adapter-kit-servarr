//! What a curator holds below its cutoff, read page by page from its own list of it.

use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{CutoffUnmet, Failure};
use serde::Deserialize;

use super::filed::Filed;
use crate::client::Servarr;

const PAGE: usize = 250;

const THE_CUTOFF_LIST: &str = "the list of what is below its cutoff";

const WITH_SERIES: &str = "&includeSeries=true";

const A_MINUTE: u64 = 60;

const A_SECOND_IN_MILLISECONDS: u64 = 1_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    #[serde(default)]
    total_records: u64,
    #[serde(default)]
    records: Vec<Held>,
}

#[derive(Deserialize, Default)]
struct Held {
    #[serde(default)]
    runtime: Option<u64>,
    #[serde(default)]
    duration: Option<u64>,
    #[serde(default)]
    series: Option<Series>,
}

#[derive(Deserialize, Default)]
struct Series {
    #[serde(default)]
    runtime: Option<u64>,
}

impl Held {
    fn seconds(&self, filed: Filed) -> u64 {
        match filed {
            Filed::Video(Kind::Tv) => self
                .runtime
                .filter(|minutes| *minutes > 0)
                .or_else(|| self.series.as_ref()?.runtime)
                .unwrap_or_default()
                .saturating_mul(A_MINUTE),
            Filed::Video(Kind::Movies) => self.runtime.unwrap_or_default().saturating_mul(A_MINUTE),
            Filed::Album => self.duration.unwrap_or_default() / A_SECOND_IN_MILLISECONDS,
        }
    }
}

/// How many items `servarr`, filing `filed`, holds below its cutoff, and their runtime.
pub(super) async fn below(servarr: &Servarr, filed: Filed) -> Result<CutoffUnmet, Failure> {
    let with = if filed == Filed::Video(Kind::Tv) {
        WITH_SERIES
    } else {
        ""
    };
    let mut held = CutoffUnmet {
        items: 0,
        seconds: 0,
    };
    for page in 1.. {
        let read: Page = servarr
            .read(
                &format!("/wanted/cutoff?page={page}&pageSize={PAGE}{with}"),
                THE_CUTOFF_LIST,
            )
            .await?;
        for one in &read.records {
            held.items += 1;
            held.seconds = held.seconds.saturating_add(one.seconds(filed));
        }
        if read.records.is_empty() || held.items >= read.total_records {
            break;
        }
    }
    Ok(held)
}

#[cfg(test)]
mod tests;
