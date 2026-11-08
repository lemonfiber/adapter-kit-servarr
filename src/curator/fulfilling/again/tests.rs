use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::digest::sha256;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Failure, Fulfilling, Grabbed};
use serde_json::json;

use crate::testing::{api, asked, films, music, sent_body, television};

const GUID: &str = "https://indexer.example/details/9?apikey=secret";

const OTHER_GUID: &str = "https://indexer.example/details/8?apikey=secret";

const QUEUED: &str = r#"{"totalRecords":2,"records":[
    {"id":50,"seriesId":4,"downloadId":"SABnzbd_nzo_0"},
    {"id":51,"seriesId":4,"downloadId":"SABnzbd_nzo_1"}
]}"#;

const SERIES: &str = r#"{"id":4,"title":"Sintel"}"#;

const FILM: &str = r#"{"id":7,"title":"Sintel"}"#;

fn grabbed_by(guid: &str) -> String {
    json!({ "records": [{ "sourceTitle": "Sintel.S01E01", "data": { "guid": guid } }] }).to_string()
}

fn refused(said: Result<impl std::fmt::Debug, Failure>, detail: &str) -> bool {
    matches!(said, Err(Failure::Refused { detail: why, .. }) if why == detail)
}

fn unsupported(said: Result<impl std::fmt::Debug, Failure>) -> bool {
    said.is_err_and(|failure| matches!(failure, Failure::Unsupported { .. }))
}

fn deleted(fake: &Fake) -> Vec<String> {
    fake.requests()
        .into_iter()
        .filter(|one| one.method == Method::Delete)
        .map(|one| one.url)
        .collect()
}

fn fingerprint() -> String {
    sha256(GUID)
}

#[tokio::test]
async fn a_queue_entry_is_removed_from_its_client_blocklisted_where_asked_and_answered_by_its_release_s_fingerprint(
) {
    for blocklist in [true, false] {
        let fake = Fake::by_rules(vec![
            (Some(Method::Get), "/queue?", Answer::reply(200, QUEUED)),
            (
                Some(Method::Get),
                "/history?",
                Answer::reply(200, grabbed_by(GUID)),
            ),
            (Some(Method::Delete), "/queue/51", Answer::reply(200, "")),
        ]);
        assert_eq!(
            television(&fake)
                .unqueue(Kind::Tv, 51, blocklist)
                .await
                .ok(),
            Some(fingerprint())
        );
        assert_eq!(
            asked(&fake),
            [
                api("/queue?page=1&pageSize=200"),
                api("/history?page=1&pageSize=1&eventType=1&downloadId=SABnzbd_nzo_1"),
                api(&format!(
                    "/queue/51?removeFromClient=true&blocklist={blocklist}&skipRedownload=true"
                )),
            ]
        );
    }
}

#[tokio::test]
async fn a_queue_entry_it_does_not_hold_is_refused_and_nothing_is_removed() {
    let fake = Fake::by_path(vec![("/queue?", Answer::reply(200, QUEUED))]);
    assert!(refused(
        films(&fake).unqueue(Kind::Movies, 9, true).await,
        "it holds no queue entry 9"
    ));
    assert!(deleted(&fake).is_empty());
}

#[tokio::test]
async fn a_queue_entry_with_no_grab_to_fingerprint_is_refused_and_nothing_is_removed() {
    for (queued, history) in [
        (r#"{"totalRecords":1,"records":[{"id":51}]}"#, "{}"),
        (QUEUED, r#"{"records":[{"data":{}},{"data":{"guid":""}}]}"#),
    ] {
        let fake = Fake::by_path(vec![
            ("/queue?", Answer::reply(200, queued)),
            ("/history?", Answer::reply(200, history)),
        ]);
        assert!(refused(
            television(&fake).unqueue(Kind::Tv, 51, true).await,
            "its history holds no grab of queue entry 51"
        ));
        assert!(deleted(&fake).is_empty());
    }
}

#[tokio::test]
async fn a_queue_entry_that_cannot_be_read_fingerprinted_or_removed_is_a_failure() {
    let fake = Fake::always(Answer::reply(500, ""));
    assert!(television(&fake).unqueue(Kind::Tv, 51, true).await.is_err());
    let fake = Fake::by_path(vec![
        ("/queue?", Answer::reply(200, QUEUED)),
        ("/history?", Answer::reply(500, "")),
    ]);
    assert!(television(&fake).unqueue(Kind::Tv, 51, true).await.is_err());
    assert!(deleted(&fake).is_empty());
    let fake = Fake::by_rules(vec![
        (Some(Method::Get), "/queue?", Answer::reply(200, QUEUED)),
        (
            Some(Method::Get),
            "/history?",
            Answer::reply(200, grabbed_by(GUID)),
        ),
        (Some(Method::Delete), "/queue/51", Answer::reply(500, "")),
    ]);
    assert!(television(&fake).unqueue(Kind::Tv, 51, true).await.is_err());
}

#[tokio::test]
async fn a_kind_it_does_not_file_is_neither_unqueued_nor_grabbed() {
    let fake = Fake::silent();
    assert!(unsupported(
        television(&fake).unqueue(Kind::Movies, 1, true).await
    ));
    assert!(unsupported(films(&fake).grab(Kind::Tv, 1, "f").await));
    assert!(unsupported(music(&fake).unqueue(Kind::Tv, 1, false).await));
    assert!(unsupported(music(&fake).grab(Kind::Movies, 1, "f").await));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn a_title_it_does_not_hold_is_refused_before_anything_is_lifted_or_searched() {
    let fake = Fake::by_path(vec![
        ("/movie/7", Answer::reply(404, "{}")),
        ("/system/status", Answer::reply(200, "{}")),
    ]);
    assert!(refused(
        films(&fake).grab(Kind::Movies, 7, &fingerprint()).await,
        "it holds no film as 7"
    ));
    assert_eq!(asked(&fake), [api("/movie/7"), api("/system/status")]);
}

const EPISODE_GRABS: &str = r#"[
    {"episodeId":30,"sourceTitle":"Sintel.S01E00","data":{"guid":"https://indexer.example/details/8?apikey=secret","indexer":"Indexer"}},
    {"episodeId":31,"sourceTitle":"Sintel.S01E01","episode":{"seasonNumber":1},"data":{"guid":"https://indexer.example/details/9?apikey=secret","indexer":"Indexer"}}
]"#;

const BLOCKED: &str = r#"{"records":[
    {"id":6,"sourceTitle":"Sintel.S01E01","indexer":"Elsewhere"},
    {"id":7,"sourceTitle":"Sintel.S01E01","indexer":"Indexer"},
    {"id":8,"sourceTitle":"Sintel.S01E00","indexer":"Indexer"}
]}"#;

const OFFERED: &str = r#"[
    {"guid":"https://indexer.example/details/8?apikey=secret","indexerId":2},
    {"guid":"https://indexer.example/details/9?apikey=secret","indexerId":3}
]"#;

#[tokio::test]
async fn an_episode_s_release_has_its_blocklist_entry_lifted_and_is_grabbed_from_its_episode_s_search(
) {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/v3/series/4",
            Answer::reply(200, SERIES),
        ),
        (
            Some(Method::Get),
            "/history/series?",
            Answer::reply(200, EPISODE_GRABS),
        ),
        (
            Some(Method::Get),
            "/blocklist?",
            Answer::reply(200, BLOCKED),
        ),
        (Some(Method::Delete), "/blocklist/7", Answer::reply(200, "")),
        (Some(Method::Get), "/release?", Answer::reply(200, OFFERED)),
        (Some(Method::Post), "/v3/release", Answer::reply(200, "{}")),
    ]);
    assert_eq!(
        television(&fake)
            .grab(Kind::Tv, 4, &fingerprint())
            .await
            .ok(),
        Some(Grabbed::Grabbed)
    );
    assert_eq!(
        asked(&fake),
        [
            api("/series/4"),
            api("/history/series?seriesId=4&eventType=1&includeEpisode=true"),
            api("/blocklist?page=1&pageSize=1000&seriesIds=4"),
            api("/blocklist/7"),
            api("/release?episodeId=31"),
            api("/release"),
        ]
    );
    assert_eq!(
        sent_body(fake.request()),
        json!({ "guid": GUID, "indexerId": 3, "seriesId": 4 })
    );
}

#[tokio::test]
async fn a_release_grabbed_for_several_episodes_is_searched_for_by_their_season() {
    let grabs = json!([
        {"episodeId":31,"sourceTitle":"Sintel.S01","episode":{"seasonNumber":1},"data":{"guid":GUID}},
        {"episodeId":32,"sourceTitle":"Sintel.S01","episode":{"seasonNumber":1},"data":{"guid":GUID}}
    ])
    .to_string();
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/v3/series/4",
            Answer::reply(200, SERIES),
        ),
        (
            Some(Method::Get),
            "/history/series?",
            Answer::reply(200, grabs),
        ),
        (Some(Method::Get), "/blocklist?", Answer::reply(200, "{}")),
        (Some(Method::Get), "/release?", Answer::reply(200, "[]")),
    ]);
    assert_eq!(
        television(&fake)
            .grab(Kind::Tv, 4, &fingerprint())
            .await
            .ok(),
        Some(Grabbed::Gone)
    );
    assert!(fake.asked_for("/release?seriesId=4&seasonNumber=1"));
    assert!(deleted(&fake).is_empty());
}

#[tokio::test]
async fn an_episode_s_release_with_no_grab_naming_its_episode_is_gone_and_nothing_is_searched() {
    for grabs in [
        "[]".to_owned(),
        json!([{"sourceTitle":"Sintel.S01E01","data":{"guid":OTHER_GUID}}]).to_string(),
        json!([{"sourceTitle":"Sintel.S01E01","data":{"guid":GUID}}]).to_string(),
        json!([
            {"sourceTitle":"Sintel.S01","data":{"guid":GUID}},
            {"sourceTitle":"Sintel.S01","data":{"guid":GUID}}
        ])
        .to_string(),
    ] {
        let fake = Fake::by_rules(vec![
            (
                Some(Method::Get),
                "/v3/series/4",
                Answer::reply(200, SERIES),
            ),
            (
                Some(Method::Get),
                "/history/series?",
                Answer::reply(200, grabs),
            ),
            (Some(Method::Get), "/blocklist?", Answer::reply(200, "{}")),
        ]);
        assert_eq!(
            television(&fake)
                .grab(Kind::Tv, 4, &fingerprint())
                .await
                .ok(),
            Some(Grabbed::Gone)
        );
        assert!(!fake.asked_for("/release"));
    }
}

#[tokio::test]
async fn a_film_s_release_is_searched_for_by_its_film_and_grabbed_where_offered() {
    let fake = Fake::by_rules(vec![
        (Some(Method::Get), "/v3/movie/7", Answer::reply(200, FILM)),
        (
            Some(Method::Get),
            "/history/movie?",
            Answer::reply(200, "[]"),
        ),
        (Some(Method::Get), "/release?", Answer::reply(200, OFFERED)),
        (Some(Method::Post), "/v3/release", Answer::reply(200, "{}")),
    ]);
    assert_eq!(
        films(&fake)
            .grab(Kind::Movies, 7, &fingerprint())
            .await
            .ok(),
        Some(Grabbed::Grabbed)
    );
    assert_eq!(
        asked(&fake),
        [
            api("/movie/7"),
            api("/history/movie?movieId=7&eventType=1"),
            api("/release?movieId=7"),
            api("/release"),
        ]
    );
    assert_eq!(
        sent_body(fake.request()),
        json!({ "guid": GUID, "indexerId": 3, "movieId": 7 })
    );
}

#[tokio::test]
async fn a_release_no_indexer_offers_any_more_is_gone_and_nothing_is_grabbed() {
    let fake = Fake::by_rules(vec![
        (Some(Method::Get), "/v3/movie/7", Answer::reply(200, FILM)),
        (
            Some(Method::Get),
            "/history/movie?",
            Answer::reply(200, "[]"),
        ),
        (
            Some(Method::Get),
            "/release?",
            Answer::reply(
                200,
                json!([{ "guid": OTHER_GUID, "indexerId": 2 }]).to_string(),
            ),
        ),
    ]);
    assert_eq!(
        films(&fake)
            .grab(Kind::Movies, 7, &fingerprint())
            .await
            .ok(),
        Some(Grabbed::Gone)
    );
    assert!(fake.requests().iter().all(|one| one.method == Method::Get));
}

#[tokio::test]
async fn a_grab_whose_history_blocklist_search_or_grab_fails_is_a_failure() {
    let failing = |at: &'static str| {
        let mut routes = vec![
            (
                Some(Method::Get),
                "/v3/series/4",
                Answer::reply(200, SERIES),
            ),
            (
                Some(Method::Get),
                "/history/series?",
                Answer::reply(200, EPISODE_GRABS),
            ),
            (
                Some(Method::Get),
                "/blocklist?",
                Answer::reply(200, BLOCKED),
            ),
            (Some(Method::Delete), "/blocklist/7", Answer::reply(200, "")),
            (Some(Method::Get), "/release?", Answer::reply(200, OFFERED)),
            (Some(Method::Post), "/v3/release", Answer::reply(200, "{}")),
        ];
        routes.insert(0, (None, at, Answer::reply(500, "")));
        Fake::by_rules(routes)
    };
    for at in [
        "/history/series?",
        "/blocklist?",
        "/blocklist/7",
        "/release?",
        "/v3/release",
    ] {
        let fake = failing(at);
        assert!(
            television(&fake)
                .grab(Kind::Tv, 4, &fingerprint())
                .await
                .is_err(),
            "{at}"
        );
    }
}
