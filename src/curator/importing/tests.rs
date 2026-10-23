use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, Importing};

use crate::testing::{api, asked, sent_body, television};

#[tokio::test]
async fn whether_it_hardlinks_is_read_from_its_media_management() {
    let fake = Fake::always(Answer::reply(200, r#"{"id":1,"copyUsingHardlinks":true}"#));
    assert_eq!(television(&fake).hardlinks().await.ok(), Some(true));
    assert_eq!(asked(&fake), [api("/config/mediamanagement")]);
}

#[tokio::test]
async fn telling_it_to_copy_keeps_every_other_setting_and_writes_to_its_id() {
    let fake = Fake::by_route(vec![
        (
            Method::Get,
            "/config/mediamanagement",
            Answer::reply(
                200,
                r#"{"id":2,"copyUsingHardlinks":true,"recycleBin":"/bin"}"#,
            ),
        ),
        (
            Method::Put,
            "/config/mediamanagement/2",
            Answer::reply(202, "{}"),
        ),
    ]);
    assert!(television(&fake).set_hardlinks(false).await.is_ok());
    assert_eq!(
        sent_body(fake.request()),
        serde_json::json!({"id":2,"copyUsingHardlinks":false,"recycleBin":"/bin"})
    );
}

#[tokio::test]
async fn settings_that_are_not_an_object_or_carry_no_id_are_refused_and_not_written() {
    for (body, said) in [
        ("[]", "the media-management settings were not an object"),
        (
            r#"{"copyUsingHardlinks":true}"#,
            "the media-management settings carry no id to write them back to",
        ),
    ] {
        let fake = Fake::always(Answer::reply(200, body));
        assert!(matches!(
            television(&fake).set_hardlinks(true).await,
            Err(Failure::Refused { detail, .. }) if detail == said
        ));
        assert_eq!(asked(&fake), [api("/config/mediamanagement")]);
    }
}
