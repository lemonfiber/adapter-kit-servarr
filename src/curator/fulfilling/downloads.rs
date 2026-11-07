use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{DownloadState, DownloadedEpisode, Downloading};

use super::super::queue::is_stuck;
use super::super::wire::QueueRecord;

const FAILED: [&str; 2] = ["failed", "failedPending"];

const STATES: [(&str, DownloadState); 5] = [
    ("queued", DownloadState::Queued),
    ("delay", DownloadState::Queued),
    ("paused", DownloadState::Paused),
    ("downloading", DownloadState::Downloading),
    ("completed", DownloadState::Completed),
];

pub(super) fn downloading(kind: Kind, record: QueueRecord) -> Option<Downloading> {
    let title_id = record.owner(kind)?;
    let state = state(&record);
    Some(Downloading {
        title_id,
        release: record.title.unwrap_or_default(),
        bytes: bytes(record.size.as_ref()),
        left: bytes(record.sizeleft.as_ref()),
        expected: record
            .estimated_completion_time
            .and_then(|at| at.parse::<jiff::Timestamp>().ok())
            .map(jiff::Timestamp::as_second),
        state,
        download_id: record.download_id.filter(|id| !id.is_empty()),
        episode: record.episode.map(|episode| DownloadedEpisode {
            season: episode.season_number,
            number: episode.episode_number,
            title: episode.title,
        }),
    })
}

fn state(record: &QueueRecord) -> DownloadState {
    let status = record.status.as_str();
    if named(&FAILED, status) || named(&FAILED, &record.tracked_download_state) {
        return DownloadState::Failed;
    }
    if is_stuck(&record.tracked_download_status) {
        return DownloadState::Warning;
    }
    STATES
        .into_iter()
        .find(|(name, _)| status.eq_ignore_ascii_case(name))
        .map_or(DownloadState::Warning, |(_, state)| state)
}

fn named(names: &[&str], said: &str) -> bool {
    names.iter().any(|name| said.eq_ignore_ascii_case(name))
}

fn bytes(said: Option<&serde_json::Number>) -> u64 {
    said.and_then(|number| {
        number.as_u64().or_else(|| {
            number
                .as_f64()
                .filter(|bytes| bytes.is_finite() && *bytes >= 0.0)
                .and_then(|bytes| format!("{bytes:.0}").parse().ok())
        })
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests;
