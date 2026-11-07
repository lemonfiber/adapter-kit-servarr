use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::DownloadState;

use super::downloading;
use crate::curator::wire::QueueRecord;

fn record(said: serde_json::Value) -> Option<QueueRecord> {
    serde_json::from_value(said).ok()
}

fn state(status: &str, tracked: &str, tracking: &str) -> Option<DownloadState> {
    record(serde_json::json!({
        "seriesId": 4,
        "status": status,
        "trackedDownloadStatus": tracked,
        "trackedDownloadState": tracking,
    }))
    .and_then(|one| downloading(Kind::Tv, one))
    .map(|one| one.state)
}

#[test]
fn where_a_download_stands_is_read_from_its_status_and_how_the_curator_tracks_it() {
    for (status, tracked, tracking, stands) in [
        ("queued", "ok", "downloading", DownloadState::Queued),
        ("Delay", "ok", "downloading", DownloadState::Queued),
        ("paused", "ok", "downloading", DownloadState::Paused),
        (
            "downloading",
            "ok",
            "downloading",
            DownloadState::Downloading,
        ),
        ("completed", "ok", "importPending", DownloadState::Completed),
        (
            "completed",
            "warning",
            "importBlocked",
            DownloadState::Warning,
        ),
        (
            "downloading",
            "error",
            "downloading",
            DownloadState::Warning,
        ),
        ("warning", "ok", "downloading", DownloadState::Warning),
        (
            "downloadClientUnavailable",
            "ok",
            "",
            DownloadState::Warning,
        ),
        ("failed", "ok", "downloading", DownloadState::Failed),
        ("completed", "error", "failedPending", DownloadState::Failed),
        ("downloading", "ok", "failed", DownloadState::Failed),
    ] {
        assert_eq!(state(status, tracked, tracking), Some(stands), "{status}");
    }
}

#[test]
fn a_size_is_read_whole_or_rounded_and_nothing_that_is_no_size() {
    let sizes = |size: serde_json::Value| {
        record(serde_json::json!({ "movieId": 7, "size": size, "sizeleft": size }))
            .and_then(|one| downloading(Kind::Movies, one))
            .map(|one| (one.bytes, one.left))
    };
    assert_eq!(sizes(serde_json::json!(12)), Some((12, 12)));
    assert_eq!(sizes(serde_json::json!(12.6)), Some((13, 13)));
    assert_eq!(sizes(serde_json::json!(-1.5)), Some((0, 0)));
    assert_eq!(sizes(serde_json::json!(-3)), Some((0, 0)));
    assert_eq!(sizes(serde_json::json!(1e30)), Some((0, 0)));
}

#[test]
fn a_download_for_no_title_of_its_kind_is_not_one() {
    let for_a_film = record(serde_json::json!({ "movieId": 7 }));
    assert!(for_a_film
        .and_then(|one| downloading(Kind::Tv, one))
        .is_none());
}
