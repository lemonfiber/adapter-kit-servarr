use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::{CutoffUnmet, Failure, Maintenance};

use crate::testing::{api, asked, films, music, television};

fn page(total: u64, records: &str) -> Answer {
    Answer::reply(
        200,
        format!(r#"{{"page":1,"pageSize":250,"totalRecords":{total},"records":[{records}]}}"#),
    )
}

#[tokio::test]
async fn an_episode_counts_its_own_runtime_or_its_series_where_it_names_none() {
    let fake = Fake::always(page(
        2,
        r#"{"runtime":45,"series":{"runtime":30}},{"runtime":0,"series":{"runtime":30}}"#,
    ));

    let held = television(&fake).cutoff_unmet(TitleKind::Tv).await;

    assert_eq!(
        held.ok(),
        Some(CutoffUnmet {
            items: 2,
            seconds: (45 + 30) * 60
        })
    );
    assert_eq!(
        asked(&fake),
        [api("/wanted/cutoff?page=1&pageSize=250&includeSeries=true")]
    );
}

#[tokio::test]
async fn a_film_counts_its_runtime_and_an_album_its_duration() {
    let fake = Fake::always(page(1, r#"{"runtime":120}"#));
    assert_eq!(
        films(&fake).cutoff_unmet(TitleKind::Movies).await.ok(),
        Some(CutoffUnmet {
            items: 1,
            seconds: 7_200
        })
    );
    assert_eq!(asked(&fake), [api("/wanted/cutoff?page=1&pageSize=250")]);

    let fake = Fake::always(page(1, r#"{"duration":2700000}"#));
    assert_eq!(
        music(&fake).cutoff_unmet(TitleKind::Music).await.ok(),
        Some(CutoffUnmet {
            items: 1,
            seconds: 2_700
        })
    );
}

#[tokio::test]
async fn every_page_is_read_until_the_list_is_whole() {
    let fake = Fake::in_turn(vec![
        page(3, r#"{"runtime":60},{"runtime":60}"#),
        page(3, r#"{"runtime":60}"#),
    ]);

    let held = films(&fake).cutoff_unmet(TitleKind::Movies).await;

    assert_eq!(
        held.ok(),
        Some(CutoffUnmet {
            items: 3,
            seconds: 3 * 3_600
        })
    );
    assert_eq!(
        asked(&fake),
        [
            api("/wanted/cutoff?page=1&pageSize=250"),
            api("/wanted/cutoff?page=2&pageSize=250")
        ]
    );
}

#[tokio::test]
async fn an_empty_list_is_nothing_below_the_cutoff_and_a_short_one_stops() {
    let fake = Fake::always(page(0, ""));
    assert_eq!(
        films(&fake).cutoff_unmet(TitleKind::Movies).await.ok(),
        Some(CutoffUnmet {
            items: 0,
            seconds: 0
        })
    );
    let fake = Fake::in_turn(vec![page(5, r#"{"runtime":10}"#), page(5, "")]);
    assert_eq!(
        films(&fake)
            .cutoff_unmet(TitleKind::Movies)
            .await
            .ok()
            .map(|held| held.items),
        Some(1)
    );
}

#[tokio::test]
async fn a_kind_it_does_not_file_is_refused_and_an_unreadable_list_is_a_failure() {
    let fake: Arc<Fake> = Fake::always(page(0, ""));
    assert!(matches!(
        television(&fake).cutoff_unmet(TitleKind::Music).await,
        Err(Failure::Unsupported { .. })
    ));
    assert!(asked(&fake).is_empty());
    let fake = Fake::always(Answer::reply(200, r#"{"records":1}"#));
    assert!(matches!(
        music(&fake).cutoff_unmet(TitleKind::Music).await,
        Err(Failure::Refused { .. })
    ));
}
