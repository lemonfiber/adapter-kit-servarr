use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{AddPlan, Added, Catalogue, CatalogueEntry, Failure};

use crate::testing::{api, asked, at, films, sent_body, television};

#[tokio::test]
async fn a_term_is_looked_up_encoded_and_each_result_read_by_its_own_catalogue() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"[{"id":0,"title":"Sintel","year":2010,"tvdbId":11},{"id":4,"title":"Sintel","tmdbId":45745}]"#,
    ));
    let found = television(&fake).lookup(Kind::Tv, "Sintel & co").await;
    assert_eq!(
        found.ok(),
        Some(vec![
            CatalogueEntry {
                title: "Sintel".to_owned(),
                year: Some(2010),
                reference: 11,
                held_as: None,
            },
            CatalogueEntry {
                title: "Sintel".to_owned(),
                year: None,
                reference: 0,
                held_as: Some(4),
            },
        ])
    );
    assert_eq!(asked(&fake), [api("/series/lookup?term=Sintel+%26+co")]);

    let fake = Fake::always(Answer::reply(
        200,
        r#"[{"id":0,"title":"Sintel","tmdbId":45745}]"#,
    ));
    let found = films(&fake).lookup(Kind::Movies, "Sintel").await;
    assert_eq!(
        found
            .ok()
            .map(|all| all.iter().map(|one| one.reference).collect::<Vec<_>>()),
        Some(vec![45745])
    );
    assert_eq!(asked(&fake), [api("/movie/lookup?term=Sintel")]);
}

#[tokio::test]
async fn an_add_is_planned_under_the_first_folder_and_the_first_profile() {
    let fake = Fake::by_path(vec![
        (
            "/rootfolder",
            Answer::reply(200, r#"[{"path":"/data/media/tv"},{"path":"/elsewhere"}]"#),
        ),
        (
            "/qualityprofile",
            Answer::reply(200, r#"[{"id":4,"name":"HD"},{"id":5,"name":"SD"}]"#),
        ),
    ]);
    assert_eq!(
        television(&fake).add_plan(Kind::Tv).await.ok(),
        Some(AddPlan {
            root_folder: "/data/media/tv".to_owned(),
            quality_profile: 4,
        })
    );
}

#[tokio::test]
async fn a_curator_with_no_folder_or_no_profile_has_no_plan() {
    let fake = Fake::by_path(vec![
        ("/rootfolder", Answer::reply(200, "[]")),
        (
            "/qualityprofile",
            Answer::reply(200, r#"[{"id":4,"name":"HD"}]"#),
        ),
    ]);
    assert!(matches!(
        television(&fake).add_plan(Kind::Tv).await,
        Err(Failure::Refused { detail, .. })
            if detail == "series has no root folder or no quality profile configured yet"
    ));
}

fn entry() -> CatalogueEntry {
    CatalogueEntry {
        title: "Sintel".to_owned(),
        year: Some(2010),
        reference: 45745,
        held_as: None,
    }
}

fn plan() -> AddPlan {
    AddPlan {
        root_folder: "/data/media".to_owned(),
        quality_profile: 4,
    }
}

#[tokio::test]
async fn each_kind_is_added_in_the_words_its_own_curator_takes_and_read_back() {
    let fake = Fake::always(Answer::reply(201, r#"{"id":9,"title":"Sintel"}"#));
    let took = television(&fake).add(Kind::Tv, &entry(), &plan()).await;
    assert_eq!(
        took.ok(),
        Some(Added {
            id: 9,
            title: "Sintel".to_owned(),
        })
    );
    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|one| one.method == Method::Post && one.url == api("/series")));
    assert_eq!(
        sent_body(sent),
        serde_json::json!({
            "title": "Sintel",
            "qualityProfileId": 4,
            "rootFolderPath": "/data/media",
            "monitored": true,
            "addOptions": { "searchForMissingEpisodes": true },
            "tvdbId": 45745,
            "seasonFolder": true,
        })
    );

    let fake = Fake::always(Answer::reply(201, r#"{"id":9,"title":"Sintel"}"#));
    assert!(films(&fake)
        .add(Kind::Movies, &entry(), &plan())
        .await
        .is_ok());
    let body = sent_body(fake.request());
    assert_eq!(at(&body, "/tmdbId"), 45745);
    assert_eq!(at(&body, "/minimumAvailability"), "released");
    assert_eq!(
        at(&body, "/addOptions"),
        serde_json::json!({ "searchForMovie": true })
    );
}

#[tokio::test]
async fn an_add_it_refuses_or_does_not_say_it_took_is_refused() {
    let fake = Fake::always(Answer::reply(
        400,
        r#"[{"errorMessage":"This series has already been added"}]"#,
    ));
    assert!(matches!(
        television(&fake).add(Kind::Tv, &entry(), &plan()).await,
        Err(Failure::Refused { detail, .. }) if detail == "HTTP 400: This series has already been added"
    ));
    let fake = Fake::always(Answer::reply(201, "taken"));
    assert!(matches!(
        television(&fake).add(Kind::Tv, &entry(), &plan()).await,
        Err(Failure::Refused { detail, .. }) if detail.starts_with("the service did not say what it took on")
    ));
}

#[tokio::test]
async fn only_an_indexer_that_would_be_searched_is_counted() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"[{"enableAutomaticSearch":true},{"enableInteractiveSearch":true},{"enableAutomaticSearch":false,"enableInteractiveSearch":false}]"#,
    ));
    assert_eq!(television(&fake).indexer_count().await.ok(), Some(2));
    assert_eq!(asked(&fake), [api("/indexer")]);
}
