use super::{describe, Endpoint, API_KEY_HEADER};
use lemonfiber_fixtures::http::Fake;
use lemonfiber_ports::http::{Method, Response};
use lemonfiber_ports::service::Failure;

fn answered(status: u16, body: impl Into<String>) -> Response {
    Response {
        status,
        headers: Vec::new(),
        body: body.into(),
    }
}

#[test]
fn a_service_quoting_the_key_back_has_it_withheld_and_keeps_its_diagnosis() {
    let response = answered(
        400,
        "invalid request: apiKey=a-fixture-and-not-a-key rejected",
    );
    assert_eq!(
        describe(&response),
        "HTTP 400: invalid request: apiKey=(set, not shown) rejected"
    );
}

#[test]
fn a_service_quoting_an_address_back_loses_the_query_and_keeps_the_host() {
    let response = answered(
        502,
        "upstream https://indexer.example/api?t=search&apikey=the-indexer-key failed",
    );
    assert_eq!(
        describe(&response),
        "HTTP 502: upstream https://indexer.example/api?(set, not shown) failed"
    );
}

#[test]
fn a_password_is_withheld_before_the_body_is_shortened_rather_than_after() {
    let body = format!(
        "{} password=a-fixture-and-not-a-password-x {}",
        "x".repeat(80),
        "y".repeat(80)
    );
    assert_eq!(
        body.chars().count(),
        201,
        "the body must be long enough to cut"
    );
    let response = answered(500, body);
    assert_eq!(
        describe(&response),
        format!(
            "HTTP 500: {} password=(set, not shown) {}",
            "x".repeat(80),
            "y".repeat(80)
        )
    );
}

#[test]
fn a_short_error_body_is_carried_whole() {
    let response = answered(500, "database is locked");
    assert_eq!(describe(&response), "HTTP 500: database is locked");
}

#[test]
fn an_empty_error_body_is_just_the_status() {
    let response = answered(503, "   ");
    assert_eq!(describe(&response), "HTTP 503");
}

#[test]
fn a_long_error_body_is_shortened_and_marked() {
    let response = answered(500, "x".repeat(500));
    let detail = describe(&response);
    assert!(
        detail.contains("..."),
        "a shortened detail is marked as shortened: {detail}"
    );
    assert!(
        detail.chars().count() < 260,
        "the 500-char body is not carried whole"
    );
}

#[test]
fn a_long_error_body_still_names_the_cause_at_its_end() {
    let response = answered(
        502,
        format!(
            "Bad Gateway. {}Connection refused by 10.0.0.5:8080.",
            "The upstream server did not answer. ".repeat(20)
        ),
    );

    let detail = describe(&response);

    assert!(detail.starts_with("HTTP 502: Bad Gateway."), "{detail}");
    assert!(
        detail.ends_with("Connection refused by 10.0.0.5:8080."),
        "{detail}"
    );
}

#[test]
fn shortening_an_error_body_uses_no_character_a_terminal_might_not_have() {
    let response = answered(500, "y".repeat(500));

    let detail = describe(&response);

    assert!(detail.is_ascii(), "{detail}");
}

#[test]
fn a_json_answer_lends_a_refusal_only_its_message_never_the_fields_beside_it() {
    let response = answered(
        400,
        r#"{"message":"bad","settings":{"password":"a-password-held"}}"#,
    );
    assert_eq!(describe(&response), "HTTP 400: bad");
    let response = answered(403, r#"{"status":false,"error":"API Key Incorrect"}"#);
    assert_eq!(describe(&response), "HTTP 403: API Key Incorrect");
    let response = answered(400, r#"{"settings":{"apikey":"a-key-held"}}"#);
    assert_eq!(describe(&response), "HTTP 400");
}

#[test]
fn an_answer_in_the_wrong_shape_is_refused_without_quoting_what_it_held() {
    let endpoint = Endpoint::new(Fake::silent(), "http://upstream", "upstream");
    let refused = endpoint.decode::<Vec<u32>>(&answered(200, r#"["a-key-held"]"#), "the list");
    assert!(matches!(refused, Err(Failure::Refused { detail, .. })
        if detail == "the list: the answer is not in the shape expected at line 1 column 13"));
}

#[test]
fn a_validation_answer_lends_a_refusal_each_failures_words_and_never_the_value_tried() {
    let response = answered(
        400,
        r#"[{"propertyName":"ApiKey","errorMessage":"Unable to connect","attemptedValue":"a-key-held"},{"propertyName":"Host","errorMessage":"Host is required"},{"propertyName":"Port"}]"#,
    );
    assert_eq!(
        describe(&response),
        "HTTP 400: Unable to connect; Host is required"
    );
}

#[test]
fn a_keyed_request_carries_the_key_and_declares_only_a_body_it_has() {
    let endpoint = Endpoint::new(Fake::silent(), "http://upstream/", "upstream");
    let bare = endpoint.keyed_request(Method::Get, "/api/v3/x", "k", None);
    assert_eq!(bare.url, "http://upstream/api/v3/x");
    assert_eq!(bare.headers, [(API_KEY_HEADER.to_owned(), "k".to_owned())]);
    let bodied = endpoint.keyed_request(Method::Post, "/api/v3/x", "k", Some("{}".to_owned()));
    assert!(bodied
        .headers
        .contains(&("Content-Type".to_owned(), "application/json".to_owned())));
    assert_eq!(endpoint.service(), "upstream");
}

#[test]
fn a_refused_key_is_unauthorised_and_anything_else_unusable_is_refused() {
    let endpoint = Endpoint::new(Fake::silent(), "http://upstream", "upstream");
    for status in [401, 403] {
        assert!(matches!(
            endpoint.refusal(&answered(status, "")),
            Failure::Unauthorised { .. }
        ));
    }
    assert!(matches!(
        endpoint.expect_success(&answered(500, "")),
        Err(Failure::Refused { .. })
    ));
    assert!(endpoint.expect_success(&answered(204, "")).is_ok());
    assert!(matches!(
        endpoint.decode::<u32>(&answered(500, ""), "a number"),
        Err(Failure::Refused { .. })
    ));
    assert!(matches!(
        endpoint.unsupported("no"),
        Failure::Unsupported { detail, .. } if detail == "no"
    ));
}
