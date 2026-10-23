use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, Importing};
use serde::Deserialize;

use super::{Curator, Music};

const MEDIA_MANAGEMENT: &str = "/config/mediamanagement";

const HARDLINKS_FIELD: &str = "copyUsingHardlinks";

const ID_FIELD: &str = "id";

const SETTINGS: &str = "the media-management settings";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaManagement {
    #[serde(default)]
    copy_using_hardlinks: bool,
}

#[async_trait]
impl<M: Music> Importing for Curator<M> {
    async fn hardlinks(&self) -> Result<bool, Failure> {
        let held: MediaManagement = self.servarr.read(MEDIA_MANAGEMENT, SETTINGS).await?;
        Ok(held.copy_using_hardlinks)
    }

    async fn set_hardlinks(&self, hardlink: bool) -> Result<(), Failure> {
        let document: serde_json::Value = self.servarr.read(MEDIA_MANAGEMENT, SETTINGS).await?;
        let serde_json::Value::Object(mut fields) = document else {
            return Err(self
                .servarr
                .endpoint()
                .refused("the media-management settings were not an object"));
        };
        let Some(id) = fields.get(ID_FIELD).and_then(serde_json::Value::as_i64) else {
            return Err(self
                .servarr
                .endpoint()
                .refused("the media-management settings carry no id to write them back to"));
        };
        fields.insert(HARDLINKS_FIELD.to_owned(), hardlink.into());
        self.servarr
            .done(
                Method::Put,
                &format!("{MEDIA_MANAGEMENT}/{id}"),
                Some(serde_json::Value::Object(fields).to_string()),
            )
            .await
    }
}

#[cfg(test)]
mod tests;
