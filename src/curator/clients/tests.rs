use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Category, ClientProbe, Credential, DownloadClient, Failure, Protocol, QualityProfile,
    RegisteredClient, RegisteredFolder,
};
use lemonfiber_ports::Client;

use super::set_field;
use crate::testing::{api, asked, at, sent_body, television};

fn client(protocol: &str, credential: Credential) -> DownloadClient {
    DownloadClient {
        name: "Fetcher".to_owned(),
        host: "fetcher".to_owned(),
        port: 8080,
        protocol: Protocol(protocol.to_owned()),
        credential,
        category: Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        },
    }
}

fn usenet() -> DownloadClient {
    client("sabnzbd", Credential::ApiKey("a-fixture-key".to_owned()))
}

fn torrent() -> DownloadClient {
    client(
        "qbittorrent",
        Credential::UserPass {
            username: "admin".to_owned(),
            password: "a-fixture-password".to_owned(),
        },
    )
}

#[tokio::test]
async fn a_usenet_client_is_saved_as_its_implementation_with_its_key_without_being_reached() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    assert!(television(&fake)
        .register_download_client(&usenet())
        .await
        .is_ok());
    let sent = fake.request();
    assert!(sent.as_ref().is_some_and(
        |one| one.method == Method::Post && one.url == api("/downloadclient?forceSave=true")
    ));
    assert_eq!(
        sent_body(sent),
        serde_json::json!({
            "enable": true,
            "protocol": "usenet",
            "name": "Fetcher",
            "implementation": "Sabnzbd",
            "configContract": "SabnzbdSettings",
            "priority": 1,
            "fields": [
                { "name": "host", "value": "fetcher" },
                { "name": "port", "value": 8080 },
                { "name": "tvCategory", "value": "tv" },
                { "name": "apiKey", "value": "a-fixture-key" },
            ],
        })
    );
}

#[tokio::test]
async fn a_torrent_client_is_posted_as_its_implementation_with_its_account() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    assert!(television(&fake)
        .register_download_client(&torrent())
        .await
        .is_ok());
    let body = sent_body(fake.request());
    assert_eq!(at(&body, "/implementation"), "QBittorrent");
    assert_eq!(at(&body, "/protocol"), "torrent");
    assert_eq!(
        at(&body, "/fields/3"),
        serde_json::json!({ "name": "username", "value": "admin" })
    );
    assert_eq!(
        at(&body, "/fields/4"),
        serde_json::json!({ "name": "password", "value": "a-fixture-password" })
    );
}

#[tokio::test]
async fn a_client_in_a_protocol_it_files_no_client_for_is_refused_before_anything_is_asked() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    let refused = television(&fake)
        .register_download_client(&client("nzbget", Credential::ApiKey("k".to_owned())))
        .await;
    assert!(matches!(refused, Err(Failure::Unsupported { detail, .. })
        if detail == "it files no download client spoken to as nzbget"));
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn an_updated_client_is_put_to_its_id_carrying_it() {
    let fake = Fake::always(Answer::reply(202, "{}"));
    assert!(television(&fake)
        .update_download_client("7", &usenet())
        .await
        .is_ok());
    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|one| one.method == Method::Put && one.url == api("/downloadclient/7")));
    assert_eq!(at(&sent_body(sent), "/id"), 7);
}

#[tokio::test]
async fn an_id_the_service_does_not_assign_is_refused_before_anything_is_asked() {
    let fake = Fake::always(Answer::reply(202, "{}"));
    let curator = television(&fake);
    for refused in [
        curator.update_download_client("../system", &usenet()).await,
        curator.set_client_field("7?x", "tvCategory", None).await,
    ] {
        assert!(matches!(refused, Err(Failure::Refused { detail, .. })
            if detail == "the download client's id is not one this service assigns"));
    }
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn the_clients_it_holds_are_read_by_where_they_are_reached() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"[
            {"id":1,"fields":[{"name":"host","value":"sabnzbd"},{"name":"port","value":8080},{"name":"tvCategory","value":"tv"}]},
            {"id":2,"fields":[{"name":"host","value":"qbittorrent"},{"name":"port","value":8081},{"name":"movieCategory","value":5}]},
            {"id":3,"fields":[{"name":"host","value":"nowhere"}]},
            {"id":4,"fields":[{"name":"host","value":"far"},{"name":"port","value":70000}]}
        ]"#,
    ));
    let held = television(&fake).download_clients().await;
    assert_eq!(
        held.ok(),
        Some(vec![
            RegisteredClient {
                id: "1".to_owned(),
                host: "sabnzbd".to_owned(),
                port: 8080,
                category: Some(Category {
                    field: "tvCategory".to_owned(),
                    value: "tv".to_owned(),
                }),
            },
            RegisteredClient {
                id: "2".to_owned(),
                host: "qbittorrent".to_owned(),
                port: 8081,
                category: None,
            },
        ])
    );
}

#[tokio::test]
async fn one_field_is_put_back_and_the_rest_of_the_client_as_it_was_read() {
    let fake = Fake::by_route(vec![
        (
            Method::Get,
            "/downloadclient/7",
            Answer::reply(
                200,
                r#"{"id":7,"name":"Fetcher","fields":[{"name":"host","value":"fetcher"},{"name":"tvCategory","value":"tv"}]}"#,
            ),
        ),
        (Method::Put, "/downloadclient/7", Answer::reply(202, "{}")),
    ]);
    assert!(television(&fake)
        .set_client_field("7", "tvCategory", Some("shows"))
        .await
        .is_ok());
    assert_eq!(
        sent_body(fake.request()),
        serde_json::json!({"id":7,"name":"Fetcher","fields":[{"name":"host","value":"fetcher"},{"name":"tvCategory","value":"shows"}]})
    );
}

#[tokio::test]
async fn a_client_with_no_settings_to_put_back_is_refused_and_not_written() {
    let fake = Fake::always(Answer::reply(200, r#"{"id":7}"#));
    let refused = television(&fake)
        .set_client_field("7", "tvCategory", None)
        .await;
    assert!(matches!(refused, Err(Failure::Refused { detail, .. })
        if detail == "the download client has no settings to put back"));
    assert_eq!(asked(&fake), [api("/downloadclient/7")]);
}

#[test]
fn a_field_is_set_where_it_is_added_where_it_is_not_and_taken_out_for_nothing() {
    let mut fields = vec![serde_json::json!({"name":"host","value":"fetcher"})];
    set_field(&mut fields, "host", Some("other"));
    set_field(&mut fields, "tvCategory", Some("tv"));
    assert_eq!(
        fields,
        [
            serde_json::json!({"name":"host","value":"other"}),
            serde_json::json!({"name":"tvCategory","value":"tv"}),
        ]
    );
    set_field(&mut fields, "host", None);
    assert_eq!(
        fields,
        [serde_json::json!({"name":"tvCategory","value":"tv"})]
    );
}

#[tokio::test]
async fn every_client_is_tested_and_each_answer_read_with_its_failures_withheld() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"[
            {"id":1,"isValid":true,"validationFailures":[]},
            {"id":2,"isValid":false,"validationFailures":[{"errorMessage":"Unable to connect"},{"errorMessage":""},{"errorMessage":"password=a-fixture-password"}]},
            {"id":3,"isValid":false}
        ]"#,
    ));
    let probes = television(&fake).test_download_clients().await;
    assert_eq!(
        probes.ok(),
        Some(vec![
            ClientProbe {
                id: "1".to_owned(),
                reachable: true,
                detail: None,
            },
            ClientProbe {
                id: "2".to_owned(),
                reachable: false,
                detail: Some("Unable to connect; password=(set, not shown)".to_owned()),
            },
            ClientProbe {
                id: "3".to_owned(),
                reachable: false,
                detail: None,
            },
        ])
    );
    assert!(
        fake.request().is_some_and(
            |one| one.method == Method::Post && one.url == api("/downloadclient/testall")
        )
    );
}

#[tokio::test]
async fn the_folders_it_files_into_are_read_with_their_ids() {
    let fake = Fake::always(Answer::reply(200, r#"[{"id":3,"path":"/data/media/tv"}]"#));
    assert_eq!(
        television(&fake).root_folders().await.ok(),
        Some(vec![RegisteredFolder {
            id: "3".to_owned(),
            path: "/data/media/tv".to_owned(),
        }])
    );
}

#[tokio::test]
async fn its_quality_profiles_are_read_leaving_out_one_with_no_usable_id_or_no_name() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"[{"id":1,"name":"HD-1080p"},{"id":-1,"name":"Broken"},{"id":2,"name":""}]"#,
    ));
    assert_eq!(
        television(&fake).quality_profiles().await.ok(),
        Some(vec![QualityProfile {
            id: 1,
            name: "HD-1080p".to_owned(),
        }])
    );
}

#[tokio::test]
async fn who_it_is_is_the_clients_identity() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"appName":"Sonarr","version":"4.0"}"#,
    ));
    assert_eq!(
        television(&fake).identity().await.ok().map(|who| who.name),
        Some("Sonarr".to_owned())
    );
}
