use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::stage::{Outcome, Stage, HISTORY_HORIZON};
use lemonfiber_ports::service::{FoundItem, ItemPart, Pipeline, QueueItem, StuckItem, TraceEvent};

use crate::testing::{api, asked, films, television};

const LIBRARY: &str = r#"[
    {"id":1,"title":"Big Buck Bunny","year":2008,"monitored":true},
    {"id":2,"title":"Sintel","year":0},
    {"id":3,"title":"Tears of Steel"}
]"#;

#[tokio::test]
async fn the_library_says_each_item_and_the_year_it_came_out_where_known() {
    let fake = Fake::always(Answer::reply(200, LIBRARY));
    let library = television(&fake).library(Kind::Tv).await;
    assert_eq!(
        library.ok().map(|all| all
            .into_iter()
            .map(|one| (one.id, one.year, one.monitored))
            .collect::<Vec<_>>()),
        Some(vec![
            (1, Some(2008), true),
            (2, None, false),
            (3, None, false)
        ])
    );
    assert_eq!(asked(&fake), [api("/series")]);
}

#[tokio::test]
async fn items_are_found_in_the_library_by_their_title_whatever_its_case() {
    let fake = Fake::always(Answer::reply(200, LIBRARY));
    assert_eq!(
        films(&fake).find_items(Kind::Movies, "BUNNY").await.ok(),
        Some(vec![FoundItem {
            id: 1,
            title: "Big Buck Bunny".to_owned(),
            year: Some(2008),
            monitored: true,
        }])
    );
    assert_eq!(asked(&fake), [api("/movie")]);
}

#[tokio::test]
async fn an_items_history_keeps_the_events_it_knows_newest_first() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"records":[
            {"eventType":"downloadFolderImported","date":"2026-10-02T10:00:00Z","episodeId":31},
            {"eventType":"episodeFileRenamed","date":"2026-10-02T09:00:00Z"},
            {"eventType":"grabbed","date":"2026-10-01T10:00:00Z","episodeId":31}
        ]}"#,
    ));
    let history = television(&fake).item_history(Kind::Tv, 7).await;
    assert_eq!(
        history.ok(),
        Some(vec![
            TraceEvent {
                outcome: Outcome::Imported,
                at: "2026-10-02T10:00:00Z".to_owned(),
                part: Some(31),
            },
            TraceEvent {
                outcome: Outcome::Grabbed,
                at: "2026-10-01T10:00:00Z".to_owned(),
                part: Some(31),
            },
        ])
    );
    assert_eq!(
        asked(&fake),
        [api(&format!(
            "/history?page=1&pageSize={HISTORY_HORIZON}&sortKey=date&sortDirection=descending&seriesIds=7"
        ))]
    );
}

#[tokio::test]
async fn a_films_history_is_asked_for_by_its_movie() {
    let fake = Fake::always(Answer::reply(200, r#"{"records":[]}"#));
    assert_eq!(
        films(&fake).item_history(Kind::Movies, 4).await.ok(),
        Some(Vec::new())
    );
    assert!(asked(&fake).iter().all(|url| url.ends_with("&movieIds=4")));
}

#[tokio::test]
async fn an_items_queue_is_read_across_every_page_for_that_item_alone() {
    let first: String = format!(
        r#"{{"totalRecords":201,"records":[{}]}}"#,
        (0..200)
            .map(|_| r#"{"seriesId":8,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"}"#)
            .collect::<Vec<_>>()
            .join(",")
    );
    let first: &'static str = Box::leak(first.into_boxed_str());
    let fake = Fake::by_path(vec![
        ("page=1&", Answer::reply(200, first)),
        (
            "page=2&",
            Answer::reply(
                200,
                r#"{"totalRecords":201,"records":[{"seriesId":7,"episodeId":31,"trackedDownloadState":"importPending","trackedDownloadStatus":"warning"}]}"#,
            ),
        ),
    ]);
    let queued = television(&fake).item_queue(Kind::Tv, 7).await;
    assert_eq!(
        queued.ok(),
        Some(vec![QueueItem {
            part: Some(31),
            stage: Stage::of_queue_state("importPending").unwrap_or(Stage::Downloading),
            stuck: true,
        }])
    );
    assert_eq!(
        asked(&fake),
        [
            api("/queue?page=1&pageSize=200"),
            api("/queue?page=2&pageSize=200")
        ]
    );
}

#[tokio::test]
async fn a_film_in_the_queue_in_a_state_it_does_not_name_is_downloading() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"totalRecords":2,"records":[{"movieId":4,"trackedDownloadState":"somethingNew","trackedDownloadStatus":"ok"},{"movieId":5}]}"#,
    ));
    assert_eq!(
        films(&fake).item_queue(Kind::Movies, 4).await.ok(),
        Some(vec![QueueItem {
            part: None,
            stage: Stage::Downloading,
            stuck: false,
        }])
    );
}

#[tokio::test]
async fn the_parts_of_a_series_are_read_in_one_season_or_all() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"[{"id":31,"seasonNumber":1,"episodeNumber":2,"title":"Two","monitored":true,"hasFile":true}]"#,
    ));
    let curator = television(&fake);
    assert_eq!(
        curator.item_parts(Kind::Tv, 7, Some(1)).await.ok(),
        Some(vec![ItemPart {
            id: 31,
            season: 1,
            number: 2,
            title: "Two".to_owned(),
            monitored: true,
            has_file: true,
        }])
    );
    assert!(curator.item_parts(Kind::Tv, 7, None).await.is_ok());
    assert_eq!(
        asked(&fake),
        [
            api("/episode?seriesId=7&seasonNumber=1"),
            api("/episode?seriesId=7")
        ]
    );
}

#[tokio::test]
async fn a_film_has_no_parts_and_nothing_is_asked_for_them() {
    let fake = Fake::silent();
    assert_eq!(
        films(&fake).item_parts(Kind::Movies, 4, None).await.ok(),
        Some(Vec::new())
    );
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn each_stuck_item_is_named_once_by_its_title() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"totalRecords":4,"records":[
            {"trackedDownloadStatus":"warning","trackedDownloadState":"importPending","series":{"title":"Show"}},
            {"trackedDownloadStatus":"Error","series":{"title":"Show"}},
            {"trackedDownloadStatus":"warning","series":{"title":""}},
            {"trackedDownloadStatus":"ok","series":{"title":"Fine"}}
        ]}"#,
    ));
    assert_eq!(
        television(&fake).stuck_items(Kind::Tv).await.ok(),
        Some(vec![StuckItem {
            title: "Show".to_owned(),
            stage: Stage::of_queue_state("importPending").unwrap_or(Stage::Downloading),
        }])
    );
    assert_eq!(
        asked(&fake),
        [api(
            "/queue?page=1&pageSize=200&includeSeries=true&includeMovie=true"
        )]
    );
}

#[tokio::test]
async fn a_stuck_film_is_named_by_its_movie() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"totalRecords":1,"records":[{"trackedDownloadStatus":"error","movie":{"title":"Sintel"}}]}"#,
    ));
    assert_eq!(
        films(&fake)
            .stuck_items(Kind::Movies)
            .await
            .ok()
            .map(|all| all.into_iter().map(|one| one.title).collect::<Vec<_>>()),
        Some(vec!["Sintel".to_owned()])
    );
}

#[tokio::test]
async fn what_cannot_be_read_is_a_failure() {
    let fake = Fake::always(Answer::reply(200, "{}"));
    let curator = television(&fake);
    assert!(curator.library(Kind::Tv).await.is_err());
    assert!(curator.find_items(Kind::Tv, "x").await.is_err());
    assert!(curator.item_parts(Kind::Tv, 7, None).await.is_err());
    let fake = Fake::always(Answer::reply(500, ""));
    let curator = television(&fake);
    assert!(curator.item_history(Kind::Tv, 7).await.is_err());
    assert!(curator.item_queue(Kind::Tv, 7).await.is_err());
    assert!(curator.stuck_items(Kind::Tv).await.is_err());
}
