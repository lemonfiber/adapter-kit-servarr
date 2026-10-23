use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::{Queue, Queued, Queues};

use super::{counted, is_stuck, HistoryRecord};
use crate::testing::{api, asked, television};

fn event(event_type: &str, episode: Option<i64>, movie: Option<i64>) -> HistoryRecord {
    HistoryRecord {
        event_type: event_type.to_owned(),
        episode_id: episode,
        movie_id: movie,
    }
}

#[test]
fn grabs_are_counted_per_item_since_it_last_imported() {
    let history = [
        event("grabbed", Some(1), None),
        event("downloadFailed", Some(1), None),
        event("grabbed", Some(1), None),
        event("downloadFolderImported", Some(1), None),
        event("grabbed", Some(1), None),
        event("grabbed", None, Some(9)),
        event("somethingElse", None, Some(9)),
        event("grabbed", None, None),
    ];
    let grabs = counted(&history);
    assert_eq!(grabs.get(&1), Some(&2));
    assert_eq!(grabs.get(&9), Some(&1));
    assert_eq!(grabs.len(), 2);
    assert!(counted(&[]).is_empty());
}

#[test]
fn a_warning_or_an_error_is_stuck_whatever_its_case() {
    assert!(is_stuck("Warning"));
    assert!(is_stuck("error"));
    assert!(!is_stuck("ok"));
}

const QUEUE: &str = r#"{"totalRecords":3,"records":[
    {"title":"Show.S01E02","trackedDownloadStatus":"warning","trackedDownloadState":"importPending","episodeId":31,"downloadId":"ABC",
     "statusMessages":[{"messages":["announce https://tracker.example/announce?passkey=a-fixture-passkey failed"]}],"errorMessage":"later"},
    {"trackedDownloadStatus":"ok","movieId":4,"errorMessage":"  ","downloadId":""},
    {"title":"Other","errorMessage":"Download failed"}
]}"#;

#[tokio::test]
async fn the_queue_carries_each_items_words_withheld_and_how_often_it_was_grabbed() {
    let fake = Fake::by_path(vec![
        ("/queue", Answer::reply(200, QUEUE)),
        (
            "/history",
            Answer::reply(
                200,
                r#"{"records":[{"eventType":"grabbed","episodeId":31},{"eventType":"grabbed","episodeId":31},{"eventType":"grabbed","episodeId":31}]}"#,
            ),
        ),
    ]);
    let queue = television(&fake).queue().await;
    assert_eq!(
        queue.ok(),
        Some(Queue {
            total: 3,
            items: vec![
                Queued {
                    title: "Show.S01E02".to_owned(),
                    status: "warning".to_owned(),
                    state: "importPending".to_owned(),
                    message: Some(
                        "announce https://tracker.example/announce?passkey=(set, not shown) failed"
                            .to_owned()
                    ),
                    download_id: Some("ABC".to_owned()),
                    grabs: 3,
                },
                Queued {
                    title: String::new(),
                    status: "ok".to_owned(),
                    state: String::new(),
                    message: None,
                    download_id: None,
                    grabs: 1,
                },
                Queued {
                    title: "Other".to_owned(),
                    status: String::new(),
                    state: String::new(),
                    message: Some("Download failed".to_owned()),
                    download_id: None,
                    grabs: 1,
                },
            ],
        })
    );
    assert_eq!(
        asked(&fake),
        [
            api("/queue?pageSize=200"),
            api("/history?page=1&pageSize=200&sortKey=date&sortDirection=descending")
        ]
    );
}

#[tokio::test]
async fn a_history_that_cannot_be_read_still_answers_with_the_queue() {
    let fake = Fake::by_path(vec![
        ("/queue", Answer::reply(200, QUEUE)),
        ("/history", Answer::reply(500, "")),
    ]);
    let queue = television(&fake).queue().await;
    assert_eq!(
        queue.ok().map(|queue| queue
            .items
            .into_iter()
            .map(|one| one.grabs)
            .collect::<Vec<_>>()),
        Some(vec![1, 1, 1])
    );
}

#[tokio::test]
async fn a_queue_that_cannot_be_read_is_a_failure() {
    let fake = Fake::silent();
    assert!(television(&fake).queue().await.is_err());
}
