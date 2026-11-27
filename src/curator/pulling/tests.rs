use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    AggregatorAt, Failure, KnownAggregator, Protocol, PullsIndexers as _,
};
use serde_json::json;

use crate::testing::{api, asked, at, films, sent_body, television};

const HYDRA: &str = "http://nzbhydra2:5076";

const MINTED: &str = "a-minted-key";

fn hydra(protocol: &str) -> AggregatorAt {
    AggregatorAt {
        name: "NZBHydra2".to_owned(),
        url: HYDRA.to_owned(),
        protocol: Protocol(protocol.to_owned()),
        key: MINTED.to_owned(),
    }
}

fn entry(id: i64, implementation: &str, url: &str, key: &str, on: bool) -> serde_json::Value {
    json!({
        "id": id,
        "name": "Somewhere",
        "implementation": implementation,
        "enableRss": on,
        "enableAutomaticSearch": false,
        "enableInteractiveSearch": false,
        "fields": [
            { "name": "baseUrl", "value": url },
            { "name": "apiPath", "value": "/api" },
            { "name": "apiKey", "value": key },
        ],
    })
}

fn schema() -> serde_json::Value {
    json!([
        { "id": 0, "implementation": "Torznab", "fields": [] },
        {
            "id": 0,
            "implementation": "Newznab",
            "configContract": "NewznabSettings",
            "enableRss": true,
            "enableAutomaticSearch": true,
            "enableInteractiveSearch": true,
            "presets": [{ "name": "Generic Newznab" }],
            "fields": [
                { "name": "baseUrl" },
                { "name": "apiPath", "value": "/api" },
                { "name": "categories", "value": [5030, 5040] },
            ],
        },
    ])
}

fn reply(body: &serde_json::Value) -> Answer {
    Answer::reply(200, body.to_string())
}

#[tokio::test]
async fn each_newznab_indexer_read_from_is_an_aggregator_with_whether_it_holds_a_key() {
    let held = json!([
        entry(3, "Newznab", HYDRA, "********", true),
        entry(4, "Newznab", "http://elsewhere/", "", true),
        entry(5, "Torznab", HYDRA, "********", true),
        entry(6, "Newznab", "http://switched-off", "********", false),
        entry(7, "Newznab", "", "********", true),
        { "implementation": "Newznab", "enableRss": true, "fields": [{ "name": "baseUrl", "value": HYDRA }] },
    ]);
    let fake = Fake::always(reply(&held));
    assert_eq!(
        television(&fake).aggregators().await.ok(),
        Some(vec![
            KnownAggregator {
                id: "3".to_owned(),
                url: HYDRA.to_owned(),
                keyed: true,
            },
            KnownAggregator {
                id: "4".to_owned(),
                url: "http://elsewhere/".to_owned(),
                keyed: false,
            },
        ])
    );
    assert_eq!(asked(&fake), [api("/indexer")]);
}

#[tokio::test]
async fn an_aggregator_read_in_another_api_is_one_it_cannot_pull_from_and_nothing_is_asked() {
    let fake = Fake::always(Answer::reply(200, "[]"));
    assert!(matches!(
        television(&fake).add_aggregator(&hydra("servarr")).await,
        Err(Failure::Unsupported { detail, .. }) if detail.contains("NZBHydra2 alone")
    ));
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn nzbhydra2_is_made_switched_off_from_the_curators_own_schema_then_switched_on_untested() {
    let fake = Fake::by_route_in_turn(vec![
        (Method::Get, "/indexer/schema", vec![reply(&schema())]),
        (Method::Get, "/indexer", vec![Answer::reply(200, "[]")]),
        (
            Method::Post,
            "/indexer",
            vec![Answer::reply(
                201,
                entry(9, "Newznab", HYDRA, "********", false).to_string(),
            )],
        ),
        (Method::Put, "/indexer/9", vec![Answer::reply(202, "{}")]),
    ]);
    assert!(films(&fake)
        .add_aggregator(&hydra("nzbhydra2"))
        .await
        .is_ok());
    assert_eq!(
        asked(&fake),
        [
            api("/indexer"),
            api("/indexer/schema"),
            api("/indexer"),
            api("/indexer/9?forceSave=true"),
        ]
    );
    let requests = fake.requests();
    let made = sent_body(requests.get(2).cloned());
    assert_eq!(
        made,
        json!({
            "id": 0,
            "name": "NZBHydra2",
            "implementation": "Newznab",
            "configContract": "NewznabSettings",
            "enableRss": false,
            "enableAutomaticSearch": false,
            "enableInteractiveSearch": false,
            "fields": [
                { "name": "baseUrl", "value": HYDRA },
                { "name": "apiPath", "value": "/api" },
                { "name": "categories", "value": [5030, 5040] },
                { "name": "apiKey", "value": MINTED },
            ],
        })
    );
    let switched = sent_body(fake.request());
    for switch in [
        "enableRss",
        "enableAutomaticSearch",
        "enableInteractiveSearch",
    ] {
        assert_eq!(at(&switched, &format!("/{switch}")), true);
    }
    assert_eq!(
        at(&switched, "/fields/2"),
        json!({ "name": "apiKey", "value": MINTED })
    );
}

#[tokio::test]
async fn one_already_at_its_address_is_handed_the_key_and_switched_on_in_place() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/indexer",
            vec![reply(&json!([
                entry(2, "Torznab", HYDRA, "********", true),
                entry(4, "Newznab", "http://nzbhydra2:5076/", "********", false),
            ]))],
        ),
        (Method::Put, "/indexer/4", vec![Answer::reply(202, "{}")]),
    ]);
    assert!(television(&fake)
        .add_aggregator(&hydra("nzbhydra2"))
        .await
        .is_ok());
    assert_eq!(
        asked(&fake),
        [api("/indexer"), api("/indexer/4?forceSave=true")]
    );
    let put = sent_body(fake.request());
    assert_eq!(at(&put, "/name"), "Somewhere");
    assert_eq!(at(&put, "/enableRss"), true);
    assert_eq!(
        at(&put, "/fields/2"),
        json!({ "name": "apiKey", "value": MINTED })
    );
}

#[tokio::test]
async fn a_curator_offering_no_newznab_indexer_cannot_pull_from_nzbhydra2() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/indexer/schema",
            vec![reply(&json!([{ "implementation": "Torznab" }]))],
        ),
        (Method::Get, "/indexer", vec![Answer::reply(200, "[]")]),
    ]);
    assert!(matches!(
        television(&fake).add_aggregator(&hydra("nzbhydra2")).await,
        Err(Failure::Unsupported { detail, .. }) if detail.contains("no Newznab indexer")
    ));
}

#[tokio::test]
async fn a_schema_with_no_settings_is_refused_before_anything_is_made() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/indexer/schema",
            vec![reply(&json!([{ "implementation": "Newznab" }]))],
        ),
        (Method::Get, "/indexer", vec![Answer::reply(200, "[]")]),
    ]);
    assert!(matches!(
        television(&fake).add_aggregator(&hydra("nzbhydra2")).await,
        Err(Failure::Refused { detail, .. }) if detail.contains("no settings")
    ));
    assert_eq!(asked(&fake), [api("/indexer"), api("/indexer/schema")]);
}

#[tokio::test]
async fn an_indexer_made_without_an_id_is_refused_and_not_switched_on() {
    let fake = Fake::by_route_in_turn(vec![
        (Method::Get, "/indexer/schema", vec![reply(&schema())]),
        (Method::Get, "/indexer", vec![Answer::reply(200, "[]")]),
        (Method::Post, "/indexer", vec![Answer::reply(201, "{}")]),
    ]);
    assert!(matches!(
        television(&fake).add_aggregator(&hydra("nzbhydra2")).await,
        Err(Failure::Refused { detail, .. }) if detail.contains("which indexer it made")
    ));
    assert_eq!(fake.requests().len(), 3);
}

#[tokio::test]
async fn a_refused_indexer_is_refused_in_the_curators_words() {
    let fake = Fake::by_route_in_turn(vec![
        (Method::Get, "/indexer/schema", vec![reply(&schema())]),
        (Method::Get, "/indexer", vec![Answer::reply(200, "[]")]),
        (
            Method::Post,
            "/indexer",
            vec![Answer::reply(
                400,
                r#"[{"propertyName":"BaseUrl","errorMessage":"Invalid Newznab URL"}]"#,
            )],
        ),
    ]);
    assert!(matches!(
        television(&fake).add_aggregator(&hydra("nzbhydra2")).await,
        Err(Failure::Refused { detail, .. }) if detail.contains("Invalid Newznab URL")
    ));
}

#[tokio::test]
async fn a_curator_reading_nzbhydra2_at_every_search_has_nothing_to_read_again() {
    let fake = Fake::always(Answer::reply(200, "[]"));
    assert!(matches!(
        television(&fake).pull().await,
        Err(Failure::Unsupported { detail, .. }) if detail.contains("nothing to read again")
    ));
    assert!(asked(&fake).is_empty());
}
