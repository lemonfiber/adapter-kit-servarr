use std::sync::Arc;

use lemonfiber_adapter_kit::credential::Holding;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::Failure;

use super::{Servarr, API_KEY_HEADER};
use crate::testing::{api, asked, servarr, BASE, KEY, SERVICE};

struct Nothing;

impl Holding for Nothing {
    fn key(&self) -> Result<String, String> {
        Err("upstream.json could not be read: not found".to_owned())
    }
}

#[tokio::test]
async fn it_answers_while_its_ping_is_answered_without_a_key() {
    let up = Fake::always(Answer::reply(200, r#"{"status":"OK"}"#));
    assert!(
        Servarr::new(up.clone(), BASE, Arc::new(Nothing), SERVICE, 3)
            .answers()
            .await
    );
    assert_eq!(asked(&up), [format!("{BASE}/ping")]);
    assert!(up.request().is_some_and(|ping| ping.headers.is_empty()));
    for down in [Fake::silent(), Fake::always(Answer::reply(503, ""))] {
        assert!(!servarr(&down).answers().await);
    }
}

#[tokio::test]
async fn without_a_key_nothing_is_asked_and_the_refusal_says_why() {
    let http = Fake::silent();
    let refused = Servarr::new(http.clone(), BASE, Arc::new(Nothing), SERVICE, 3)
        .ask(Method::Get, "/system/status", None)
        .await;
    assert!(matches!(refused, Err(Failure::Refused { detail, .. })
        if detail == "this adapter holds no key for sonarr: upstream.json could not be read: not found"));
    assert!(asked(&http).is_empty());
}

#[tokio::test]
async fn every_call_rides_the_versioned_api_with_the_key() {
    let fake = Fake::always(Answer::reply(200, "{}"));
    assert!(servarr(&fake)
        .done(Method::Post, "/command", Some("{}".to_owned()))
        .await
        .is_ok());
    let sent = fake.request();
    assert_eq!(
        sent.as_ref().map(|one| one.url.clone()),
        Some(api("/command"))
    );
    assert!(sent.is_some_and(|one| one
        .headers
        .contains(&(API_KEY_HEADER.to_owned(), KEY.to_owned()))));
}

#[tokio::test]
async fn a_versioned_prefix_it_does_not_serve_is_a_release_it_does_not_speak() {
    let fake = Fake::always(Answer::reply(404, ""));
    let refused = servarr(&fake).ask(Method::Get, "/rootfolder", None).await;
    assert!(matches!(refused, Err(Failure::Unsupported { detail, .. })
        if detail == "there is no /api/v3/rootfolder; it may be a release this adapter does not speak"));
}

#[tokio::test]
async fn nothing_answering_is_unavailable() {
    assert!(matches!(
        servarr(&Fake::silent()).identity().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_write_it_refuses_is_refused() {
    let fake = Fake::always(Answer::reply(
        400,
        r#"[{"errorMessage":"Path is required"}]"#,
    ));
    assert!(matches!(
        servarr(&fake).done(Method::Post, "/rootfolder", None).await,
        Err(Failure::Refused { detail, .. }) if detail == "HTTP 400: Path is required"
    ));
}

#[tokio::test]
async fn every_validation_failure_is_said_and_nothing_beside_it() {
    let fake = Fake::always(Answer::reply(
        400,
        r#"[{"propertyName":"Path","errorMessage":"Path is required","attemptedValue":"a-fixture-key"},{"errorMessage":"Name is taken"},{"severity":"error"}]"#,
    ));
    assert!(matches!(
        servarr(&fake).done(Method::Post, "/rootfolder", None).await,
        Err(Failure::Refused { detail, .. })
            if detail == "HTTP 400: Path is required; Name is taken"
    ));
}

#[tokio::test]
async fn a_refusal_in_its_own_message_is_said_as_it_is() {
    let fake = Fake::always(Answer::reply(409, r#"{"message":"Already exists"}"#));
    assert!(matches!(
        servarr(&fake).done(Method::Post, "/series", None).await,
        Err(Failure::Refused { detail, .. }) if detail == "HTTP 409: Already exists"
    ));
}

#[tokio::test]
async fn a_list_it_answers_is_read_as_it_is() {
    let fake = Fake::always(Answer::reply(200, r#"[{"errorMessage":"kept"}]"#));
    let read: Vec<serde_json::Value> = servarr(&fake)
        .read("/health", "the health")
        .await
        .unwrap_or_default();
    assert_eq!(read, [serde_json::json!({ "errorMessage": "kept" })]);
}

#[tokio::test]
async fn its_identity_is_its_instance_name_or_its_app_name_and_its_version() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"instanceName":"Shows","appName":"Sonarr","version":"4.0.15.2941"}"#,
    ));
    let identity = servarr(&fake).identity().await;
    assert_eq!(
        identity.ok().map(|who| (who.name, who.version)),
        Some(("Shows".to_owned(), "4.0.15.2941".to_owned()))
    );
    assert_eq!(asked(&fake), [api("/system/status")]);

    let fake = Fake::always(Answer::reply(
        200,
        r#"{"appName":"Radarr","version":"5.0"}"#,
    ));
    assert_eq!(
        servarr(&fake).identity().await.ok().map(|who| who.name),
        Some("Radarr".to_owned())
    );
}

#[tokio::test]
async fn a_status_naming_neither_itself_nor_its_version_is_refused() {
    for body in [r#"{"version":"4.0"}"#, r#"{"appName":"Sonarr"}"#] {
        let fake = Fake::always(Answer::reply(200, body));
        assert!(matches!(
            servarr(&fake).identity().await,
            Err(Failure::Refused { detail, .. })
                if detail == "the service named neither itself nor its version"
        ));
    }
}

#[tokio::test]
async fn an_unreadable_status_names_what_could_not_be_read() {
    let fake = Fake::always(Answer::reply(200, "not json"));
    assert!(matches!(
        servarr(&fake).identity().await,
        Err(Failure::Refused { detail, .. })
            if detail.starts_with("the status could not be read: the answer is not in the shape expected")
    ));
}

#[test]
fn it_names_the_service_every_failure_is_blamed_on() {
    assert_eq!(servarr(&Fake::silent()).service(), SERVICE);
}
