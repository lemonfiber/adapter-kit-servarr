use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::{Kind, TitleKind};
use lemonfiber_ports::service::{CuratedSeason, CuratedTitle, Failure, Fulfilling, TitleRequest};

use crate::testing::{api, asked, films, music, sent_body, television};

mod requesting;

const SERIES: &str = r#"{"id":4,"title":"Sintel","year":2010,"tvdbId":11,"monitored":true,
    "statistics":{"episodeFileCount":3,"totalEpisodeCount":9},
    "seasons":[{"seasonNumber":0,"monitored":false},
               {"seasonNumber":1,"monitored":true,"statistics":{"episodeFileCount":3,"totalEpisodeCount":8}}]}"#;

const FILM: &str =
    r#"{"id":7,"title":"Sintel","year":0,"tmdbId":45745,"monitored":false,"hasFile":true}"#;

fn series() -> CuratedTitle {
    CuratedTitle {
        reference: 11,
        id: Some(4),
        title: "Sintel".to_owned(),
        year: Some(2010),
        monitored: true,
        files: 3,
        seasons: vec![
            CuratedSeason {
                number: 0,
                monitored: false,
                files: 0,
                episodes: 0,
            },
            CuratedSeason {
                number: 1,
                monitored: true,
                files: 3,
                episodes: 8,
            },
        ],
    }
}

fn film() -> CuratedTitle {
    CuratedTitle {
        reference: 45745,
        id: Some(7),
        title: "Sintel".to_owned(),
        year: None,
        monitored: false,
        files: 1,
        seasons: Vec::new(),
    }
}

fn refused(said: Result<impl std::fmt::Debug, Failure>, detail: &str) -> bool {
    matches!(said, Err(Failure::Refused { detail: why, .. }) if why == detail)
}

#[tokio::test]
async fn a_title_it_holds_is_catalogued_as_it_holds_it() {
    let fake = Fake::always(Answer::reply(
        200,
        format!(r#"[{{"id":9,"tvdbId":12}},{SERIES}]"#),
    ));
    assert_eq!(
        television(&fake).catalogued(Kind::Tv, 11).await.ok(),
        Some(Some(series()))
    );
    assert_eq!(asked(&fake), [api("/series?tvdbId=11")]);

    let fake = Fake::always(Answer::reply(200, format!("[{FILM}]")));
    assert_eq!(
        films(&fake).catalogued(Kind::Movies, 45745).await.ok(),
        Some(Some(film()))
    );
    assert_eq!(asked(&fake), [api("/movie?tmdbId=45745")]);
}

#[tokio::test]
async fn a_title_it_holds_that_cannot_be_read_is_refused() {
    let fake = Fake::always(Answer::reply(200, r#"[{"tvdbId":11,"monitored":"yes"}]"#));
    assert!(refused(
        television(&fake).catalogued(Kind::Tv, 11).await,
        "the title it holds could not be read"
    ));
}

#[tokio::test]
async fn a_title_it_does_not_hold_is_catalogued_as_its_catalogue_knows_it_unmonitored() {
    let fake = Fake::by_path(vec![
        (
            "/series/lookup",
            Answer::reply(
                200,
                r#"[{"title":"Other","tvdbId":12},
                    {"title":"Sintel","year":2010,"tvdbId":11,"monitored":true,
                     "seasons":[{"seasonNumber":1,"monitored":true}]}]"#,
            ),
        ),
        ("/series", Answer::reply(200, "[]")),
    ]);
    assert_eq!(
        television(&fake).catalogued(Kind::Tv, 11).await.ok(),
        Some(Some(CuratedTitle {
            reference: 11,
            id: None,
            title: "Sintel".to_owned(),
            year: Some(2010),
            monitored: false,
            files: 0,
            seasons: vec![CuratedSeason {
                number: 1,
                monitored: false,
                files: 0,
                episodes: 0,
            }],
        }))
    );
    assert_eq!(
        asked(&fake),
        [
            api("/series?tvdbId=11"),
            api("/series/lookup?term=tvdb%3A11")
        ]
    );

    let fake = Fake::by_path(vec![
        (
            "/movie/lookup",
            Answer::reply(200, r#"[{"title":"Other","tmdbId":1}]"#),
        ),
        ("/movie", Answer::reply(200, "[]")),
    ]);
    assert_eq!(
        films(&fake).catalogued(Kind::Movies, 45745).await.ok(),
        Some(None)
    );
    assert!(fake.asked_for("/movie/lookup?term=tmdb%3A45745"));
}

#[tokio::test]
async fn every_title_it_holds_is_answered() {
    let fake = Fake::always(Answer::reply(200, format!("[{SERIES},{SERIES}]")));
    assert_eq!(
        television(&fake).titles(TitleKind::Tv).await.ok(),
        Some(vec![series(), series()])
    );
    assert_eq!(asked(&fake), [api("/series")]);

    let fake = Fake::always(Answer::reply(200, format!("[{FILM}]")));
    assert_eq!(
        films(&fake).titles(TitleKind::Movies).await.ok(),
        Some(vec![film()])
    );
    assert_eq!(asked(&fake), [api("/movie")]);
}

#[tokio::test]
async fn a_title_it_holds_is_answered_by_its_id_and_one_it_does_not_hold_is_none() {
    let fake = Fake::always(Answer::reply(200, FILM));
    assert_eq!(
        films(&fake).held_title(Kind::Movies, 7).await.ok(),
        Some(Some(film()))
    );
    assert_eq!(asked(&fake), [api("/movie/7")]);

    let fake = Fake::by_path(vec![
        ("/series/5", Answer::reply(404, r#"{"title":"Not Found"}"#)),
        ("/system/status", Answer::reply(200, "{}")),
    ]);
    assert_eq!(
        television(&fake).held_title(Kind::Tv, 5).await.ok(),
        Some(None)
    );
}

#[tokio::test]
async fn a_title_it_holds_is_searched_for_as_the_command_for_its_kind() {
    let fake = Fake::by_route(vec![
        (Method::Get, "/series/4", Answer::reply(200, SERIES)),
        (Method::Post, "/command", Answer::reply(201, "{}")),
    ]);
    assert!(television(&fake).search(TitleKind::Tv, 4).await.is_ok());
    assert_eq!(asked(&fake), [api("/series/4"), api("/command")]);
    assert_eq!(
        sent_body(fake.request()),
        serde_json::json!({ "name": "MissingEpisodeSearch", "seriesId": 4 })
    );

    let fake = Fake::by_route(vec![
        (Method::Get, "/movie/7", Answer::reply(200, FILM)),
        (Method::Post, "/command", Answer::reply(201, "{}")),
    ]);
    assert!(films(&fake).search(TitleKind::Movies, 7).await.is_ok());
    assert_eq!(
        sent_body(fake.request()),
        serde_json::json!({ "name": "MoviesSearch", "movieIds": [7] })
    );
}

#[tokio::test]
async fn a_title_it_holds_is_removed_with_its_files_and_excluded_from_nothing() {
    let fake = Fake::by_route(vec![
        (Method::Get, "/series/4", Answer::reply(200, SERIES)),
        (Method::Delete, "/series/4", Answer::reply(200, "{}")),
    ]);
    assert!(television(&fake).remove(Kind::Tv, 4).await.is_ok());
    assert_eq!(
        asked(&fake),
        [
            api("/series/4"),
            api("/series/4?deleteFiles=true&addImportListExclusion=false")
        ]
    );
    assert!(fake
        .request()
        .is_some_and(|sent| sent.method == Method::Delete));

    let fake = Fake::by_route(vec![
        (Method::Get, "/movie/7", Answer::reply(200, FILM)),
        (Method::Delete, "/movie/7", Answer::reply(200, "{}")),
    ]);
    assert!(films(&fake).remove(Kind::Movies, 7).await.is_ok());
    assert!(fake.asked_for("/movie/7?deleteFiles=true&addImportExclusion=false"));
}

#[tokio::test]
async fn a_title_it_does_not_hold_is_neither_searched_for_nor_removed() {
    let fake = Fake::by_path(vec![
        ("/series/5", Answer::reply(404, r#"{"title":"Not Found"}"#)),
        ("/system/status", Answer::reply(200, "{}")),
    ]);
    let tv = television(&fake);
    assert!(refused(
        tv.search(TitleKind::Tv, 5).await,
        "it holds no series as 5"
    ));
    assert!(refused(
        tv.remove(Kind::Tv, 5).await,
        "it holds no series as 5"
    ));
    assert!(fake
        .requests()
        .iter()
        .all(|sent| sent.method == Method::Get));
}

#[tokio::test]
async fn a_kind_it_does_not_file_is_unsupported_before_anything_is_asked() {
    let fake = Fake::always(Answer::reply(200, "[]"));
    let request = TitleRequest {
        reference: 11,
        root_folder: "/tv".to_owned(),
        quality_profile: 1,
        seasons: Vec::new(),
        anime: false,
        later_seasons: false,
        search: false,
    };
    let curators = [television(&fake), films(&fake), music(&fake)];
    for (curator, kind) in curators.iter().zip([Kind::Movies, Kind::Tv, Kind::Tv]) {
        let said = |answer: Result<(), Failure>| {
            matches!(answer, Err(Failure::Unsupported { detail, .. })
                if detail == format!("it files no {}", kind.noun()))
        };
        assert!(said(curator.catalogued(kind, 11).await.map(drop)));
        assert!(said(curator.titles(kind.into()).await.map(drop)));
        assert!(said(curator.held_title(kind, 4).await.map(drop)));
        assert!(said(curator.request(kind, &request).await.map(drop)));
        assert!(said(curator.search(kind.into(), 4).await));
        assert!(said(curator.remove(kind, 4).await));
        assert!(said(curator.downloading(kind.into()).await.map(drop)));
    }
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn what_it_downloads_is_read_from_its_queue_with_each_episode() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"totalRecords":2,"records":[
            {"id":1786,"title":"Sintel.S01E02","seriesId":4,"size":1000,"sizeleft":250.4,"status":"downloading",
             "trackedDownloadStatus":"ok","trackedDownloadState":"downloading",
             "estimatedCompletionTime":"2024-05-01T10:20:30Z","downloadId":"SAB_1",
             "episode":{"seasonNumber":1,"episodeNumber":2,"title":"Two"}},
            {"title":"Unmatched","status":"downloading"}
        ]}"#,
    ));
    let downloading = television(&fake).downloading(TitleKind::Tv).await;
    assert_eq!(
        downloading.ok().map(|all| all
            .into_iter()
            .map(|one| serde_json::to_value(one).unwrap_or_default())
            .collect::<Vec<_>>()),
        Some(vec![serde_json::json!({
            "title_id": 4,
            "release": "Sintel.S01E02",
            "bytes": 1000,
            "left": 250,
            "expected": 1_714_558_830,
            "state": "downloading",
            "download_id": "SAB_1",
            "queued": 1786,
            "episode": { "season": 1, "number": 2, "title": "Two" },
        })])
    );
    assert_eq!(
        asked(&fake),
        [api("/queue?page=1&pageSize=200&includeEpisode=true")]
    );

    let fake = Fake::always(Answer::reply(
        200,
        r#"{"totalRecords":1,"records":[{"movieId":7,"downloadId":"","estimatedCompletionTime":"soon"}]}"#,
    ));
    let downloading = films(&fake).downloading(TitleKind::Movies).await;
    assert_eq!(
        downloading.ok().map(|all| all
            .into_iter()
            .map(|one| (
                one.title_id,
                one.release,
                one.bytes,
                one.expected,
                one.download_id,
                one.queued
            ))
            .collect::<Vec<_>>()),
        Some(vec![(7, String::new(), 0, None, None, None)])
    );
    assert_eq!(asked(&fake), [api("/queue?page=1&pageSize=200")]);
}

#[tokio::test]
async fn a_queue_it_cannot_read_is_refused() {
    let fake = Fake::always(Answer::reply(200, r#"{"records":"none"}"#));
    assert!(matches!(
        films(&fake).downloading(TitleKind::Movies).await,
        Err(Failure::Refused { .. })
    ));
}
