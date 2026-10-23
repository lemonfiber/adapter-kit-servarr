use async_trait::async_trait;
use lemonfiber_manifest::ApiKind;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    ClientProbe, Credential, DownloadClient, Failure, Identity, QualityProfile, RegisteredClient,
    RegisteredFolder, RootFolder,
};
use lemonfiber_ports::Client;

use super::wire::{ClientResource, FolderResource, ProfileResource, TestResource};
use super::{Curator, Music, QUALITY_PROFILES, ROOT_FOLDERS};

const DOWNLOAD_CLIENTS: &str = "/downloadclient";

const TEST_ALL: &str = "/downloadclient/testall";

const SAVED_UNREACHED: &str = "/downloadclient?forceSave=true";

const FIELDS: &str = "fields";

const NAME: &str = "name";

const VALUE: &str = "value";

const PRIORITY: u8 = 1;

struct Filed {
    spoken: ApiKind,
    implementation: &'static str,
    settings: &'static str,
    grouped_as: &'static str,
}

const FILED: [Filed; 2] = [
    Filed {
        spoken: ApiKind::Sabnzbd,
        implementation: "Sabnzbd",
        settings: "SabnzbdSettings",
        grouped_as: "usenet",
    },
    Filed {
        spoken: ApiKind::Qbittorrent,
        implementation: "QBittorrent",
        settings: "QBittorrentSettings",
        grouped_as: "torrent",
    },
];

#[async_trait]
impl<M: Music> Client for Curator<M> {
    async fn identity(&self) -> Result<Identity, Failure> {
        self.servarr.identity().await
    }

    async fn register_download_client(&self, client: &DownloadClient) -> Result<(), Failure> {
        let body = self.download_client_body(client, None)?;
        self.servarr
            .done(Method::Post, SAVED_UNREACHED, Some(body))
            .await
    }

    async fn download_clients(&self) -> Result<Vec<RegisteredClient>, Failure> {
        let clients: Vec<ClientResource> = self
            .servarr
            .read(DOWNLOAD_CLIENTS, "the download clients")
            .await?;
        Ok(clients
            .into_iter()
            .filter_map(ClientResource::endpoint)
            .collect())
    }

    async fn update_download_client(
        &self,
        id: &str,
        client: &DownloadClient,
    ) -> Result<(), Failure> {
        let numeric = self.assigned(id)?;
        let body = self.download_client_body(client, Some(numeric))?;
        self.servarr
            .done(
                Method::Put,
                &format!("{DOWNLOAD_CLIENTS}/{numeric}"),
                Some(body),
            )
            .await
    }

    async fn set_client_field(
        &self,
        id: &str,
        field: &str,
        value: Option<&str>,
    ) -> Result<(), Failure> {
        let numeric = self.assigned(id)?;
        let path = format!("{DOWNLOAD_CLIENTS}/{numeric}");
        let mut document: serde_json::Value =
            self.servarr.read(&path, "the download client").await?;
        let Some(fields) = document
            .get_mut(FIELDS)
            .and_then(serde_json::Value::as_array_mut)
        else {
            return Err(self
                .servarr
                .endpoint()
                .refused("the download client has no settings to put back"));
        };
        set_field(fields, field, value);
        self.servarr
            .done(Method::Put, &path, Some(document.to_string()))
            .await
    }

    async fn test_download_clients(&self) -> Result<Vec<ClientProbe>, Failure> {
        let response = self.servarr.ask(Method::Post, TEST_ALL, None).await?;
        let results: Vec<TestResource> = self.servarr.endpoint().decode(
            &response,
            "the download-client test results could not be read",
        )?;
        Ok(results.into_iter().map(TestResource::probe).collect())
    }

    async fn register_root_folder(&self, folder: &RootFolder) -> Result<(), Failure> {
        let mut body = self.music.root_folder(&self.servarr, folder).await?;
        body.insert("path".to_owned(), serde_json::json!(folder.path));
        self.servarr
            .done(
                Method::Post,
                ROOT_FOLDERS,
                Some(serde_json::Value::Object(body).to_string()),
            )
            .await
    }

    async fn root_folders(&self) -> Result<Vec<RegisteredFolder>, Failure> {
        let folders: Vec<FolderResource> =
            self.servarr.read(ROOT_FOLDERS, "the root folders").await?;
        Ok(folders
            .into_iter()
            .map(|folder| RegisteredFolder {
                id: folder.id.to_string(),
                path: folder.path,
            })
            .collect())
    }

    async fn quality_profiles(&self) -> Result<Vec<QualityProfile>, Failure> {
        let held: Vec<ProfileResource> = self
            .servarr
            .read(QUALITY_PROFILES, "the quality profiles")
            .await?;
        Ok(held
            .into_iter()
            .filter_map(|profile| {
                u32::try_from(profile.id)
                    .ok()
                    .filter(|_| !profile.name.is_empty())
                    .map(|id| QualityProfile {
                        id,
                        name: profile.name,
                    })
            })
            .collect())
    }
}

impl<M: Music> Curator<M> {
    fn assigned(&self, id: &str) -> Result<i64, Failure> {
        id.parse::<i64>().map_err(|_| {
            self.servarr
                .endpoint()
                .refused("the download client's id is not one this service assigns")
        })
    }

    fn download_client_body(
        &self,
        client: &DownloadClient,
        id: Option<i64>,
    ) -> Result<String, Failure> {
        let Some(filed) = FILED
            .iter()
            .find(|filed| filed.spoken.name() == client.protocol.0)
        else {
            return Err(self.servarr.endpoint().unsupported(&format!(
                "it files no download client spoken to as {}",
                client.protocol.0
            )));
        };
        let mut fields = vec![
            serde_json::json!({ NAME: "host", VALUE: client.host }),
            serde_json::json!({ NAME: "port", VALUE: client.port }),
            serde_json::json!({ NAME: client.category.field, VALUE: client.category.value }),
        ];
        match &client.credential {
            Credential::ApiKey(key) => {
                fields.push(serde_json::json!({ NAME: "apiKey", VALUE: key }));
            }
            Credential::UserPass { username, password } => {
                fields.push(serde_json::json!({ NAME: "username", VALUE: username }));
                fields.push(serde_json::json!({ NAME: "password", VALUE: password }));
            }
        }
        let mut document = serde_json::Map::from_iter([
            ("enable".to_owned(), serde_json::json!(true)),
            ("protocol".to_owned(), serde_json::json!(filed.grouped_as)),
            (NAME.to_owned(), serde_json::json!(client.name)),
            (
                "implementation".to_owned(),
                serde_json::json!(filed.implementation),
            ),
            (
                "configContract".to_owned(),
                serde_json::json!(filed.settings),
            ),
            ("priority".to_owned(), serde_json::json!(PRIORITY)),
            (FIELDS.to_owned(), serde_json::Value::Array(fields)),
        ]);
        if let Some(id) = id {
            document.insert("id".to_owned(), serde_json::json!(id));
        }
        Ok(serde_json::Value::Object(document).to_string())
    }
}

fn set_field(fields: &mut Vec<serde_json::Value>, field: &str, value: Option<&str>) {
    let named = |held: &serde_json::Value| {
        held.get(NAME).and_then(serde_json::Value::as_str) == Some(field)
    };
    let Some(value) = value else {
        fields.retain(|held| !named(held));
        return;
    };
    if let Some(held) = fields
        .iter_mut()
        .find(|held| named(held))
        .and_then(serde_json::Value::as_object_mut)
    {
        held.insert(VALUE.to_owned(), serde_json::json!(value));
        return;
    }
    fields.push(serde_json::json!({ NAME: field, VALUE: value }));
}

#[cfg(test)]
mod tests;
