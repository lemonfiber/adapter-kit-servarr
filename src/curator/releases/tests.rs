use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{QualityReleases, ReleaseProbe};

use crate::testing::{api, asked, films, television};

fn probing(releases: &'static str) -> std::sync::Arc<Fake> {
    Fake::by_path(vec![
        (
            "/wanted/missing",
            Answer::reply(200, r#"{"records":[{"id":31},{"id":32}]}"#),
        ),
        ("/release", Answer::reply(200, releases)),
    ])
}

#[tokio::test]
async fn the_first_wanted_item_is_searched_and_what_it_would_grab_said() {
    for (releases, said) in [
        ("[]", ReleaseProbe::NoneFound),
        (
            r#"[{"rejections":["too small"]},{"rejections":[]}]"#,
            ReleaseProbe::Matching,
        ),
        (r#"[{"rejections":["too small"]}]"#, ReleaseProbe::NoneMatch),
    ] {
        let fake = probing(releases);
        assert_eq!(
            television(&fake).probe_releases(Kind::Tv).await.ok(),
            Some(said)
        );
        assert_eq!(
            asked(&fake),
            [
                api("/wanted/missing?page=1&pageSize=1"),
                api("/release?episodeId=31")
            ]
        );
    }
    let fake = probing("[]");
    assert!(films(&fake).probe_releases(Kind::Movies).await.is_ok());
    assert!(asked(&fake).contains(&api("/release?movieId=31")));
}

#[tokio::test]
async fn nothing_wanted_searches_nothing() {
    let fake = Fake::always(Answer::reply(200, r#"{"records":[]}"#));
    assert_eq!(
        television(&fake).probe_releases(Kind::Tv).await.ok(),
        Some(ReleaseProbe::NothingWanted)
    );
    assert_eq!(asked(&fake).len(), 1);
}
