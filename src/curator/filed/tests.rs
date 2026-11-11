use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::digest::sha256;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::stage::{Outcome, HISTORY_HORIZON};
use lemonfiber_ports::service::{
    CuratedTitle, Failure, FoundItem, Fulfilling, Grabbed, Pipeline, QualityReleases, QueueItem,
    Recounting, ReleaseProbe, StuckItem, TraceEvent,
};
use serde_json::json;

use crate::curator::Files;
use crate::testing::{api, asked, curator, films, music, sent_body, television};

const ALBUMS: &str = r#"[
    {"id":7,"title":"Kind of Blue","artistId":3,"releaseDate":"1959-08-17T00:00:00Z","monitored":true,
     "statistics":{"trackFileCount":5,"trackCount":5},"artist":{"artistName":"Miles Davis"}},
    {"id":8,"title":"Bitches Brew","artistId":3,"monitored":false,"artist":{"artistName":"Miles Davis"}}
]"#;

const ALBUM: &str = r#"{"id":7,"title":"Kind of Blue","artistId":3}"#;

const GUID: &str = "https://indexer.example/details/9?apikey=secret";

fn unsupported(said: Result<impl std::fmt::Debug, Failure>, detail: &str) -> bool {
    matches!(said, Err(Failure::Unsupported { detail: why, .. }) if why == detail)
}

#[tokio::test]
async fn the_library_of_music_is_every_album_by_its_own_id_and_the_year_it_came_out() {
    let fake = Fake::always(Answer::reply(200, ALBUMS));
    assert_eq!(
        music(&fake).library(TitleKind::Music).await.ok(),
        Some(vec![
            FoundItem {
                id: 7,
                title: "Kind of Blue".to_owned(),
                year: Some(1959),
                monitored: true,
            },
            FoundItem {
                id: 8,
                title: "Bitches Brew".to_owned(),
                year: None,
                monitored: false,
            },
        ])
    );
    assert_eq!(asked(&fake), [api("/album")]);
}

#[tokio::test]
async fn an_album_is_found_by_its_own_title_and_never_by_its_artist() {
    let fake = Fake::always(Answer::reply(200, ALBUMS));
    let curator = music(&fake);
    assert_eq!(
        curator
            .find_items(TitleKind::Music, "kind of BLUE")
            .await
            .ok()
            .map(|found| found.into_iter().map(|one| one.id).collect::<Vec<_>>()),
        Some(vec![7])
    );
    assert_eq!(
        curator
            .find_items(TitleKind::Music, "Miles Davis")
            .await
            .ok(),
        Some(Vec::new())
    );
}

#[tokio::test]
async fn an_album_s_history_is_read_by_its_own_id_and_names_no_part() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"records":[
            {"eventType":"trackFileDeleted","date":"2026-10-03T10:00:00Z","albumId":7,"trackId":70},
            {"eventType":"downloadImported","date":"2026-10-02T10:00:00Z","albumId":7,"trackId":0},
            {"eventType":"trackFileImported","date":"2026-10-02T09:59:00Z","albumId":7,"trackId":70},
            {"eventType":"grabbed","date":"2026-10-01T10:00:00Z","albumId":7,"trackId":0}
        ]}"#,
    ));
    let event = |outcome, at: &str| TraceEvent {
        outcome,
        at: at.to_owned(),
        part: None,
    };
    assert_eq!(
        music(&fake).item_history(TitleKind::Music, 7).await.ok(),
        Some(vec![
            event(Outcome::Removed, "2026-10-03T10:00:00Z"),
            event(Outcome::Imported, "2026-10-02T10:00:00Z"),
            event(Outcome::Grabbed, "2026-10-01T10:00:00Z"),
        ])
    );
    assert_eq!(
        asked(&fake),
        [api(&format!(
            "/history?page=1&pageSize={HISTORY_HORIZON}&sortKey=date&sortDirection=descending&albumId=7"
        ))]
    );
}

#[tokio::test]
async fn an_album_s_queue_is_its_own_entries_and_never_another_album_s_by_the_artist() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"totalRecords":2,"records":[
            {"id":51,"artistId":3,"albumId":7,"trackedDownloadStatus":"warning","trackedDownloadState":"downloading"},
            {"id":52,"artistId":3,"albumId":8,"trackedDownloadStatus":"ok","trackedDownloadState":"downloading"}
        ]}"#,
    ));
    assert_eq!(
        music(&fake)
            .item_queue(TitleKind::Music, 7)
            .await
            .ok()
            .map(|queued| queued
                .into_iter()
                .map(|one: QueueItem| (one.part, one.stuck))
                .collect::<Vec<_>>()),
        Some(vec![(None, true)])
    );
    assert_eq!(asked(&fake), [api("/queue?page=1&pageSize=200")]);
}

#[tokio::test]
async fn an_album_has_no_parts_and_nothing_is_asked_for_them() {
    let fake = Fake::always(Answer::reply(200, "[]"));
    assert_eq!(
        music(&fake)
            .item_parts(TitleKind::Music, 7, Some(1))
            .await
            .ok(),
        Some(Vec::new())
    );
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn a_stuck_album_is_named_by_its_own_title_under_its_own_id() {
    let fake = Fake::by_path(vec![
        (
            "/queue?",
            Answer::reply(
                200,
                r#"{"totalRecords":2,"records":[
                    {"id":51,"artistId":3,"albumId":7,"downloadId":"SAB_1","trackedDownloadStatus":"warning",
                     "artist":{"artistName":"Miles Davis"},"album":{"title":"Kind of Blue"}},
                    {"id":52,"artistId":3,"trackedDownloadStatus":"warning","artist":{"artistName":"Miles Davis"}}
                ]}"#,
            ),
        ),
        (
            "/history?",
            Answer::reply(
                200,
                json!({ "records": [{ "data": { "guid": GUID } }] }).to_string(),
            ),
        ),
    ]);
    assert_eq!(
        music(&fake).stuck_items(TitleKind::Music).await.ok(),
        Some(vec![StuckItem {
            title: "Kind of Blue".to_owned(),
            stage: lemonfiber_ports::service::stage::Stage::Downloading,
            queued: Some(51),
            title_id: Some(7),
            release: Some(sha256(GUID)),
        }])
    );
    assert_eq!(
        asked(&fake),
        [
            api("/queue?page=1&pageSize=200&includeAlbum=true"),
            api("/history?page=1&pageSize=1&eventType=1&downloadId=SAB_1")
        ]
    );
}

#[tokio::test]
async fn the_titles_of_music_are_its_albums_each_by_its_own_id_with_its_track_files() {
    let fake = Fake::always(Answer::reply(200, ALBUMS));
    assert_eq!(
        music(&fake).titles(TitleKind::Music).await.ok(),
        Some(vec![
            CuratedTitle {
                reference: 0,
                id: Some(7),
                title: "Kind of Blue".to_owned(),
                year: Some(1959),
                monitored: true,
                files: 5,
                seasons: Vec::new(),
            },
            CuratedTitle {
                reference: 0,
                id: Some(8),
                title: "Bitches Brew".to_owned(),
                year: None,
                monitored: false,
                files: 0,
                seasons: Vec::new(),
            },
        ])
    );
    assert_eq!(asked(&fake), [api("/album")]);
}

#[tokio::test]
async fn an_album_it_holds_is_searched_for_by_the_album_search_command() {
    let fake = Fake::by_rules(vec![
        (Some(Method::Get), "/album/7", Answer::reply(200, ALBUM)),
        (Some(Method::Post), "/command", Answer::reply(201, "{}")),
    ]);
    assert!(music(&fake).search(TitleKind::Music, 7).await.is_ok());
    assert_eq!(asked(&fake), [api("/album/7"), api("/command")]);
    assert_eq!(
        sent_body(fake.request()),
        json!({ "name": "AlbumSearch", "albumIds": [7] })
    );
}

#[tokio::test]
async fn an_album_it_does_not_hold_is_refused_and_not_searched() {
    let fake = Fake::by_path(vec![
        ("/album/9", Answer::reply(404, "{}")),
        ("/system/status", Answer::reply(200, "{}")),
    ]);
    assert!(matches!(
        music(&fake).search(TitleKind::Music, 9).await,
        Err(Failure::Refused { detail, .. }) if detail == "it holds no album as 9"
    ));
    assert!(!fake.asked_for("/command"));
}

#[tokio::test]
async fn what_it_downloads_of_music_is_each_album_s_queue_entry() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"totalRecords":2,"records":[
            {"id":51,"albumId":7,"title":"Miles.Davis-Kind.of.Blue-FLAC","size":100,"sizeleft":60,"status":"downloading"},
            {"id":52,"artistId":3,"title":"Unmatched","status":"downloading"}
        ]}"#,
    ));
    assert_eq!(
        music(&fake)
            .downloading(TitleKind::Music)
            .await
            .ok()
            .map(|all| all
                .into_iter()
                .map(|one| (one.title_id, one.release, one.left, one.queued, one.episode))
                .collect::<Vec<_>>()),
        Some(vec![(
            7,
            "Miles.Davis-Kind.of.Blue-FLAC".to_owned(),
            60,
            Some(51),
            None
        )])
    );
    assert_eq!(asked(&fake), [api("/queue?page=1&pageSize=200")]);
}

#[tokio::test]
async fn an_album_s_queue_entry_is_dropped_as_any_other() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/queue?",
            Answer::reply(
                200,
                r#"{"totalRecords":1,"records":[{"id":51,"albumId":7,"downloadId":"SAB_1"}]}"#,
            ),
        ),
        (
            Some(Method::Get),
            "/history?",
            Answer::reply(
                200,
                json!({ "records": [{ "data": { "guid": GUID } }] }).to_string(),
            ),
        ),
        (Some(Method::Delete), "/queue/51", Answer::reply(200, "")),
    ]);
    assert_eq!(
        music(&fake).unqueue(TitleKind::Music, 51, true).await.ok(),
        Some(sha256(GUID))
    );
    assert!(fake.asked_for("/queue/51?removeFromClient=true&blocklist=true&skipRedownload=true"));
}

const ALBUM_GRABS: &str = r#"[
    {"albumId":7,"sourceTitle":"Miles.Davis-Kind.of.Blue-FLAC","data":{"guid":"https://indexer.example/details/9?apikey=secret","indexer":"Indexer"}},
    {"albumId":7,"sourceTitle":"Miles.Davis-Kind.of.Blue-MP3","data":{"guid":"https://indexer.example/details/8?apikey=secret","indexer":"Indexer"}}
]"#;

const ALBUMS_BLOCKED: &str = r#"{"totalRecords":3,"records":[
    {"id":5,"artistId":3,"albumIds":[8],"sourceTitle":"Miles.Davis-Kind.of.Blue-FLAC","indexer":"Indexer"},
    {"id":6,"artistId":3,"albumIds":[7],"sourceTitle":"Miles.Davis-Kind.of.Blue-FLAC","indexer":"Elsewhere"},
    {"id":7,"artistId":3,"albumIds":[7],"sourceTitle":"Miles.Davis-Kind.of.Blue-FLAC","indexer":"Indexer"}
]}"#;

#[tokio::test]
async fn an_album_s_release_has_its_own_blocklist_entry_lifted_and_is_grabbed_from_its_search() {
    let fake = Fake::by_rules(vec![
        (Some(Method::Get), "/album/7", Answer::reply(200, ALBUM)),
        (
            Some(Method::Get),
            "/history/artist?",
            Answer::reply(200, ALBUM_GRABS),
        ),
        (
            Some(Method::Get),
            "/blocklist?",
            Answer::reply(200, ALBUMS_BLOCKED),
        ),
        (Some(Method::Delete), "/blocklist/7", Answer::reply(200, "")),
        (
            Some(Method::Get),
            "/release?",
            Answer::reply(200, json!([{ "guid": GUID, "indexerId": 3 }]).to_string()),
        ),
        (Some(Method::Post), "/release", Answer::reply(200, "{}")),
    ]);
    assert_eq!(
        music(&fake)
            .grab(TitleKind::Music, 7, &sha256(GUID))
            .await
            .ok(),
        Some(Grabbed::Grabbed)
    );
    assert_eq!(
        asked(&fake),
        [
            api("/album/7"),
            api("/history/artist?artistId=3&albumId=7&eventType=1"),
            api("/blocklist?page=1&pageSize=1000"),
            api("/blocklist/7"),
            api("/release?albumId=7"),
            api("/release"),
        ]
    );
    assert_eq!(
        sent_body(fake.request()),
        json!({ "guid": GUID, "indexerId": 3, "albumId": 7 })
    );
}

#[tokio::test]
async fn an_album_s_blocklist_is_read_page_by_page_to_its_end() {
    let page = json!({
        "totalRecords": 1001,
        "records": (0..1000).map(|id| json!({ "id": id, "albumIds": [8] })).collect::<Vec<_>>(),
    })
    .to_string();
    let fake = Fake::by_rules(vec![
        (Some(Method::Get), "/album/7", Answer::reply(200, ALBUM)),
        (
            Some(Method::Get),
            "/history/artist?",
            Answer::reply(200, ALBUM_GRABS),
        ),
        (
            Some(Method::Get),
            "/blocklist?page=1&",
            Answer::reply(200, page),
        ),
        (
            Some(Method::Get),
            "/blocklist?page=2&",
            Answer::reply(200, ALBUMS_BLOCKED),
        ),
        (Some(Method::Delete), "/blocklist/7", Answer::reply(200, "")),
        (Some(Method::Get), "/release?", Answer::reply(200, "[]")),
    ]);
    assert_eq!(
        music(&fake)
            .grab(TitleKind::Music, 7, &sha256(GUID))
            .await
            .ok(),
        Some(Grabbed::Gone)
    );
    assert!(fake.asked_for("/blocklist?page=2&pageSize=1000"));
    assert!(fake.asked_for("/blocklist/7"));
    assert!(!fake.asked_for("/blocklist/5"));
}

#[tokio::test]
async fn the_releases_probed_for_music_are_those_of_its_first_wanted_album() {
    let fake = Fake::by_path(vec![
        (
            "/wanted/missing",
            Answer::reply(200, r#"{"records":[{"id":7}]}"#),
        ),
        ("/release?", Answer::reply(200, r#"[{"rejections":[]}]"#)),
    ]);
    assert_eq!(
        music(&fake).probe_releases(TitleKind::Music).await.ok(),
        Some(ReleaseProbe::Matching)
    );
    assert_eq!(
        asked(&fake),
        [
            api("/wanted/missing?page=1&pageSize=1"),
            api("/release?albumId=7")
        ]
    );
}

#[tokio::test]
async fn a_kind_it_does_not_file_is_refused_as_unsupported_before_anything_is_asked() {
    let fake = Fake::always(Answer::reply(200, "[]"));
    let books = curator(
        &fake,
        Files {
            kind: TitleKind::Books,
            ..Files::MUSIC
        },
    );
    assert!(unsupported(
        music(&fake).library(TitleKind::Books).await,
        "it files no book"
    ));
    assert!(unsupported(
        books.titles(TitleKind::Books).await,
        "it files no book"
    ));
    assert!(unsupported(
        music(&fake).downloading(TitleKind::Tv).await,
        "it files no series"
    ));
    assert!(unsupported(
        television(&fake).stuck_items(TitleKind::Music).await,
        "it files no album"
    ));
    assert!(unsupported(
        films(&fake).grab(TitleKind::Music, 7, "f").await,
        "it files no album"
    ));
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn no_curator_tells_the_core_what_its_upstream_did_yet() {
    let fake = Fake::always(Answer::reply(200, "{}"));
    for curator in [television(&fake), films(&fake), music(&fake)] {
        assert!(matches!(
            curator
                .events(Some("https://core"), Some("pin"), Some("key"))
                .await,
            Err(Failure::Unsupported { .. })
        ));
    }
    assert!(asked(&fake).is_empty());
}
