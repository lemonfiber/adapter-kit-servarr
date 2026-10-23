use async_trait::async_trait;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::{Format, Kind};
use lemonfiber_ports::service::{
    Carrying, Catalogue, Failure, Maintenance, MusicQuality, Pipeline, QualityReleases, Record,
    RootFolder,
};
use lemonfiber_ports::Client;

use super::{Curator, Files, Music};
use crate::client::Servarr;
use crate::testing::{api, asked, at, films, music, sent_body, servarr, television};

fn unsupported(refused: Result<impl std::fmt::Debug, Failure>, said: &str) -> bool {
    matches!(refused, Err(Failure::Unsupported { detail, .. }) if detail == said)
}

#[tokio::test]
async fn an_upgrade_search_is_posted_as_the_command_for_the_kind_it_files() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    assert!(television(&fake).search_upgrades(Kind::Tv).await.is_ok());
    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|one| one.method == Method::Post && one.url == api("/command")));
    assert_eq!(at(&sent_body(sent), "/name"), "CutoffUnmetEpisodeSearch");

    let fake = Fake::always(Answer::reply(201, "{}"));
    assert!(films(&fake).search_upgrades(Kind::Movies).await.is_ok());
    assert_eq!(
        at(&sent_body(fake.request()), "/name"),
        "CutoffUnmetMoviesSearch"
    );
}

#[tokio::test]
async fn a_kind_it_does_not_file_is_refused_before_anything_is_asked() {
    let fake = Fake::always(Answer::reply(200, "[]"));
    assert!(unsupported(
        television(&fake).search_upgrades(Kind::Movies).await,
        "it files no film"
    ));
    assert!(unsupported(
        films(&fake).lookup(Kind::Tv, "x").await,
        "it files no series"
    ));
    assert!(unsupported(
        music(&fake).library(Kind::Tv).await,
        "it files no series"
    ));
    assert!(unsupported(
        music(&fake).probe_releases(Kind::Movies).await,
        "it files no film"
    ));
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn a_record_it_does_not_hold_is_refused_before_anything_is_asked() {
    let fake = Fake::always(Answer::reply(200, "[]"));
    assert!(unsupported(
        television(&fake).records(Record::Film).await,
        "it holds no films"
    ));
    assert!(unsupported(
        music(&fake).records(Record::Indexer).await,
        "it holds no indexers"
    ));
    assert!(asked(&fake).is_empty());
}

#[tokio::test]
async fn a_curator_of_no_music_refuses_a_music_format_and_registers_a_folder_by_its_path() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    let curator = television(&fake);
    assert!(unsupported(
        curator.apply_music_format(Format::Lossless).await,
        "it files no music"
    ));
    assert!(asked(&fake).is_empty());
    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    assert!(curator.register_root_folder(&folder).await.is_ok());
    assert_eq!(
        sent_body(fake.request()),
        serde_json::json!({ "path": "/data/media/tv" })
    );
}

struct Shelved;

#[async_trait]
impl Music for Shelved {
    async fn apply(&self, servarr: &Servarr, format: Format) -> Result<(), Failure> {
        servarr
            .done(Method::Put, &format!("/format/{}", format.label()), None)
            .await
    }

    async fn root_folder(
        &self,
        _servarr: &Servarr,
        folder: &RootFolder,
    ) -> Result<serde_json::Map<String, serde_json::Value>, Failure> {
        Ok(serde_json::Map::from_iter([(
            "name".to_owned(),
            serde_json::json!(folder.media_type),
        )]))
    }
}

#[tokio::test]
async fn a_curator_of_music_answers_what_its_music_does() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    let curator = Curator::new(servarr(&fake), Files::MUSIC, Shelved);
    assert!(curator.apply_music_format(Format::Lossless).await.is_ok());
    let folder = RootFolder {
        path: "/data/media/music".to_owned(),
        media_type: "music".to_owned(),
    };
    assert!(curator.register_root_folder(&folder).await.is_ok());
    assert_eq!(
        asked(&fake),
        [
            api(&format!("/format/{}", Format::Lossless.label())),
            api("/rootfolder")
        ]
    );
    assert_eq!(
        sent_body(fake.request()),
        serde_json::json!({ "name": "music", "path": "/data/media/music" })
    );
    assert_eq!(curator.servarr().service(), "sonarr");
}
