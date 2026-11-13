use lemonfiber_fixtures::http::Fake;
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::Pipeline;

use crate::testing::television;

#[tokio::test]
async fn a_queue_entry_with_no_download_id_asks_no_history() {
    let fake = Fake::always(lemonfiber_fixtures::http::Answer::reply(
        200,
        r#"{"totalRecords":2,"records":[
            {"id":1,"trackedDownloadStatus":"warning","series":{"title":"One"}},
            {"id":2,"downloadId":"","trackedDownloadStatus":"warning","series":{"title":"Two"}}
        ]}"#,
    ));
    assert_eq!(
        television(&fake)
            .stuck_items(TitleKind::Tv)
            .await
            .ok()
            .map(|all| all.into_iter().map(|one| one.release).collect::<Vec<_>>()),
        Some(vec![None, None])
    );
    assert!(!fake.asked_for("/history"));
}
