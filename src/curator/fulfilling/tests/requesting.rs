use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::{Method, Request};
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Fulfilling, TitleRequest};
use serde_json::json;

use super::{film, refused, series, FILM, SERIES};
use crate::testing::{api, asked, at, films, sent_body, television};

const HELD_SERIES: &str = r#"[{"id":4,"title":"Sintel","tvdbId":11,"monitored":false,"path":"/data/tv/Sintel","tags":[2],
    "seasons":[{"seasonNumber":0,"monitored":false},{"seasonNumber":1,"monitored":false},{"seasonNumber":2,"monitored":true}]}]"#;

fn asking(reference: i64) -> TitleRequest {
    TitleRequest {
        reference,
        root_folder: "/data/tv".to_owned(),
        quality_profile: 4,
        seasons: vec![1],
        anime: false,
        later_seasons: false,
        search: false,
    }
}

fn adding(lookup: &'static str, posted: &'static str) -> std::sync::Arc<Fake> {
    Fake::by_rules(vec![
        (Some(Method::Get), "/lookup", Answer::reply(200, lookup)),
        (
            Some(Method::Get),
            "Id=",
            Answer::reply(200, r#"[{"id":9,"tvdbId":99,"tmdbId":99}]"#),
        ),
        (
            Some(Method::Get),
            "/qualityprofile",
            Answer::reply(200, r#"[{"id":3},{"id":4}]"#),
        ),
        (
            Some(Method::Get),
            "/rootfolder",
            Answer::reply(
                200,
                r#"[{"id":1,"path":"/elsewhere"},{"id":2,"path":"/data/tv/"}]"#,
            ),
        ),
        (Some(Method::Post), "/", Answer::reply(201, posted)),
    ])
}

fn sent(fake: &Fake, method: Method) -> Vec<Request> {
    fake.requests()
        .into_iter()
        .filter(|one| one.method == method)
        .collect()
}

#[tokio::test]
async fn a_series_it_does_not_hold_is_added_monitoring_the_seasons_named_and_no_other() {
    let fake = adding(
        r#"[{"title":"Sintel","year":2010,"tvdbId":11,
             "seasons":[{"seasonNumber":0,"monitored":false},{"seasonNumber":1,"monitored":true},{"seasonNumber":2,"monitored":true}]}]"#,
        SERIES,
    );
    let request = TitleRequest {
        anime: true,
        later_seasons: true,
        search: true,
        ..asking(11)
    };
    assert_eq!(
        television(&fake).request(Kind::Tv, &request).await.ok(),
        Some(series())
    );
    assert_eq!(
        asked(&fake),
        [
            api("/series?tvdbId=11"),
            api("/qualityprofile"),
            api("/rootfolder"),
            api("/series/lookup?term=tvdb%3A11"),
            api("/series"),
        ]
    );
    assert_eq!(
        sent_body(fake.request()),
        json!({
            "title": "Sintel",
            "tvdbId": 11,
            "qualityProfileId": 4,
            "rootFolderPath": "/data/tv/",
            "monitored": true,
            "seasonFolder": true,
            "seriesType": "anime",
            "monitorNewItems": "all",
            "addOptions": { "searchForMissingEpisodes": true },
            "seasons": [
                { "seasonNumber": 0, "monitored": false },
                { "seasonNumber": 1, "monitored": true },
                { "seasonNumber": 2, "monitored": false },
            ],
        })
    );

    let fake = adding(r#"[{"title":"Sintel","tvdbId":11}]"#, SERIES);
    assert!(television(&fake)
        .request(Kind::Tv, &asking(11))
        .await
        .is_ok());
    let posted = sent_body(fake.request());
    assert_eq!(at(&posted, "/seriesType"), "standard");
    assert_eq!(at(&posted, "/monitorNewItems"), "none");
    assert_eq!(
        at(&posted, "/addOptions"),
        json!({ "searchForMissingEpisodes": false })
    );
    assert_eq!(at(&posted, "/seasons"), json!([]));
}

#[tokio::test]
async fn a_film_it_does_not_hold_is_added_monitored_with_no_seasons() {
    let fake = adding(r#"[{"title":"Sintel","tmdbId":45745}]"#, FILM);
    assert_eq!(
        films(&fake)
            .request(Kind::Movies, &asking(45745))
            .await
            .ok(),
        Some(film())
    );
    assert!(fake.asked_for("/movie?tmdbId=45745"));
    assert!(fake.asked_for("/movie/lookup?term=tmdb%3A45745"));
    assert_eq!(
        sent_body(fake.request()),
        json!({
            "title": "Sintel",
            "tmdbId": 45745,
            "qualityProfileId": 4,
            "rootFolderPath": "/data/tv/",
            "monitored": true,
            "minimumAvailability": "released",
            "addOptions": { "searchForMovie": false },
        })
    );
}

#[tokio::test]
async fn an_addition_at_a_profile_or_a_folder_it_does_not_hold_or_of_nothing_catalogued_is_refused()
{
    let lookup = r#"[{"title":"Sintel","tvdbId":11}]"#;
    for (request, said) in [
        (
            TitleRequest {
                quality_profile: 9,
                ..asking(11)
            },
            "it holds no quality profile 9",
        ),
        (
            TitleRequest {
                root_folder: "/data".to_owned(),
                ..asking(11)
            },
            "it holds no root folder /data",
        ),
        (asking(12), "its catalogue holds no series as 12"),
    ] {
        let fake = adding(lookup, SERIES);
        assert!(refused(
            television(&fake).request(Kind::Tv, &request).await,
            said
        ));
        assert!(sent(&fake, Method::Post).is_empty());
    }
}

#[tokio::test]
async fn a_series_it_holds_is_monitored_with_the_seasons_named_and_their_episodes_then_searched() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/series?",
            Answer::reply(200, HELD_SERIES),
        ),
        (Some(Method::Put), "/series/4", Answer::reply(202, SERIES)),
        (
            Some(Method::Get),
            "/episode?seriesId=4",
            Answer::reply(
                200,
                r#"[{"id":1,"seasonNumber":1,"monitored":false},{"id":2,"seasonNumber":1,"monitored":true},
                    {"id":3,"seasonNumber":2,"monitored":false},{"id":5,"seasonNumber":3,"monitored":false}]"#,
            ),
        ),
        (
            Some(Method::Put),
            "/episode/monitor",
            Answer::reply(202, "{}"),
        ),
        (Some(Method::Post), "/command", Answer::reply(201, "{}")),
    ]);
    let request = TitleRequest {
        root_folder: String::new(),
        quality_profile: 0,
        seasons: vec![1, 2],
        search: true,
        ..asking(11)
    };
    assert_eq!(
        television(&fake).request(Kind::Tv, &request).await.ok(),
        Some(series())
    );
    assert_eq!(
        asked(&fake),
        [
            api("/series?tvdbId=11"),
            api("/series/4"),
            api("/episode?seriesId=4"),
            api("/episode/monitor"),
            api("/command"),
        ]
    );
    let puts = sent(&fake, Method::Put);
    assert_eq!(
        puts.first().cloned().map(Some).map(sent_body),
        Some(json!({
            "id": 4,
            "title": "Sintel",
            "tvdbId": 11,
            "monitored": true,
            "path": "/data/tv/Sintel",
            "tags": [2],
            "seasons": [
                { "seasonNumber": 0, "monitored": false },
                { "seasonNumber": 1, "monitored": true },
                { "seasonNumber": 2, "monitored": true },
            ],
        }))
    );
    assert_eq!(
        puts.get(1).cloned().map(Some).map(sent_body),
        Some(json!({ "episodeIds": [1, 3], "monitored": true }))
    );
    assert_eq!(
        sent_body(fake.request()),
        json!({ "name": "MissingEpisodeSearch", "seriesId": 4 })
    );
}

#[tokio::test]
async fn a_series_it_holds_whose_episodes_are_monitored_already_has_only_itself_written() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/series?",
            Answer::reply(200, HELD_SERIES),
        ),
        (Some(Method::Put), "/series/4", Answer::reply(202, SERIES)),
        (
            Some(Method::Get),
            "/episode?seriesId=4",
            Answer::reply(200, r#"[{"id":1,"seasonNumber":1,"monitored":true}]"#),
        ),
    ]);
    assert!(television(&fake)
        .request(Kind::Tv, &asking(11))
        .await
        .is_ok());
    assert_eq!(sent(&fake, Method::Put).len(), 1);
    assert!(sent(&fake, Method::Post).is_empty());

    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/series?",
            Answer::reply(200, HELD_SERIES),
        ),
        (Some(Method::Put), "/series/4", Answer::reply(202, SERIES)),
    ]);
    let request = TitleRequest {
        seasons: Vec::new(),
        ..asking(11)
    };
    assert!(television(&fake).request(Kind::Tv, &request).await.is_ok());
    assert!(!fake.asked_for("/episode"));
}

#[tokio::test]
async fn a_film_it_holds_is_monitored_then_searched_and_nothing_else_is_written() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/movie?",
            Answer::reply(
                200,
                r#"[{"id":7,"tmdbId":45745,"monitored":false,"path":"/films/Sintel"}]"#,
            ),
        ),
        (Some(Method::Put), "/movie/7", Answer::reply(202, FILM)),
        (Some(Method::Post), "/command", Answer::reply(201, "{}")),
    ]);
    let request = TitleRequest {
        seasons: Vec::new(),
        search: true,
        ..asking(45745)
    };
    assert_eq!(
        films(&fake).request(Kind::Movies, &request).await.ok(),
        Some(film())
    );
    assert_eq!(
        sent(&fake, Method::Put)
            .first()
            .cloned()
            .map(Some)
            .map(sent_body),
        Some(json!({ "id": 7, "tmdbId": 45745, "monitored": true, "path": "/films/Sintel" }))
    );
    assert_eq!(
        sent_body(fake.request()),
        json!({ "name": "MoviesSearch", "movieIds": [7] })
    );
    assert!(!fake.asked_for("/episode"));
}

#[tokio::test]
async fn a_monitoring_it_does_not_answer_with_the_title_is_refused() {
    let fake = Fake::by_rules(vec![
        (
            Some(Method::Get),
            "/series?",
            Answer::reply(200, HELD_SERIES),
        ),
        (
            Some(Method::Put),
            "/series/4",
            Answer::reply(202, r#""none""#),
        ),
    ]);
    assert!(matches!(
        television(&fake).request(Kind::Tv, &asking(11)).await,
        Err(lemonfiber_ports::service::Failure::Refused { detail, .. })
            if detail.starts_with("the service did not say what it monitors")
    ));
}
