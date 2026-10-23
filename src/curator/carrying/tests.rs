use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Carried, Carrying, Failure, Record};
use serde_json::{Map, Value};

use super::named;
use crate::testing::{api, asked, music, sent_body, television};

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[test]
fn a_record_is_named_by_whichever_word_its_service_uses_and_a_title_first() {
    assert_eq!(
        named(&object(&serde_json::json!({"name":"Indexer"}))),
        "Indexer"
    );
    assert_eq!(
        named(&object(&serde_json::json!({"artistName":"Band"}))),
        "Band"
    );
    assert_eq!(
        named(&object(&serde_json::json!({"name":"other","title":"Show"}))),
        "Show"
    );
    assert_eq!(named(&object(&serde_json::json!({"title":3}))), "");
}

const PROFILES: &str = r#"[{"id":1,"name":"Any"},{"id":4,"name":"HD-1080p"},{"id":-2,"name":"Broken"},{"id":6,"name":""}]"#;

#[tokio::test]
async fn the_records_it_holds_travel_by_profile_name_and_folder_path() {
    let fake = Fake::by_path(vec![
        (
            "/series",
            Answer::reply(
                200,
                r#"[{"id":7,"title":"Show","qualityProfileId":4,"rootFolderPath":"/data/media/tv"},{"id":8,"title":"","qualityProfileId":1},3,{"id":9,"title":"Other","qualityProfileId":99}]"#,
            ),
        ),
        ("/qualityprofile", Answer::reply(200, PROFILES)),
    ]);
    let held = television(&fake).records(Record::Series).await;
    assert_eq!(
        held.ok().map(|all| all
            .into_iter()
            .map(|one| (one.name, one.profile, one.folder))
            .collect::<Vec<_>>()),
        Some(vec![
            (
                "Show".to_owned(),
                Some("HD-1080p".to_owned()),
                Some("/data/media/tv".to_owned())
            ),
            ("Other".to_owned(), None, None),
        ])
    );
    assert_eq!(asked(&fake), [api("/series"), api("/qualityprofile")]);
}

fn following(profile: Option<&str>, rest: &str) -> Carried {
    Carried {
        name: "Band".to_owned(),
        profile: profile.map(str::to_owned),
        folder: Some("/data/media/music".to_owned()),
        rest: rest.to_owned(),
    }
}

#[tokio::test]
async fn a_record_is_put_under_this_services_own_number_for_its_profile() {
    let fake = Fake::by_route(vec![
        (Method::Get, "/qualityprofile", Answer::reply(200, PROFILES)),
        (Method::Post, "/artist", Answer::reply(201, "{}")),
    ]);
    let carried = following(
        Some("HD-1080p"),
        r#"{"id":12,"artistName":"Band","qualityProfileId":1,"rootFolderPath":"/old","monitored":true}"#,
    );
    assert!(music(&fake).carry(Record::Artist, &carried).await.is_ok());
    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|one| one.method == Method::Post && one.url == api("/artist")));
    assert_eq!(
        sent_body(sent),
        serde_json::json!({"artistName":"Band","monitored":true,"qualityProfileId":4,"rootFolderPath":"/data/media/music"})
    );
}

#[tokio::test]
async fn a_record_naming_no_profile_crosses_without_asking_for_any() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    let carried = Carried {
        folder: None,
        ..following(None, r#"{"artistName":"Band"}"#)
    };
    assert!(music(&fake).carry(Record::Artist, &carried).await.is_ok());
    assert_eq!(asked(&fake), [api("/artist")]);
    assert_eq!(
        sent_body(fake.request()),
        serde_json::json!({"artistName":"Band"})
    );
}

#[tokio::test]
async fn a_profile_this_service_lacks_is_refused_rather_than_guessed() {
    let fake = Fake::always(Answer::reply(200, PROFILES));
    let refused = music(&fake)
        .carry(Record::Artist, &following(Some("Lossless"), "{}"))
        .await;
    assert!(matches!(refused, Err(Failure::Unsupported { detail, .. })
        if detail == "no quality profile called Lossless"));
    assert_eq!(asked(&fake), [api("/qualityprofile")]);
}

#[tokio::test]
async fn a_record_that_cannot_be_read_back_is_not_put_to_the_service() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    let refused = music(&fake)
        .carry(Record::Artist, &following(None, "[1]"))
        .await;
    assert!(matches!(refused, Err(Failure::Unsupported { detail, .. })
        if detail == "a carried record of artists could not be read back"));
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn a_record_the_service_will_not_take_is_refused() {
    let fake = Fake::always(Answer::reply(400, r#"[{"errorMessage":"Artist exists"}]"#));
    let refused = music(&fake)
        .carry(Record::Artist, &following(None, "{}"))
        .await;
    assert!(matches!(refused, Err(Failure::Refused { detail, .. })
        if detail == "HTTP 400: Artist exists"));
}
