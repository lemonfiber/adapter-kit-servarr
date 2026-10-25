use std::collections::BTreeMap;

use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Carried, Carrying, Failure, Record};
use serde_json::{Map, Value};

use super::wire::ProfileResource;
use super::{Curator, Music, QUALITY_PROFILES};

const PROFILE_FIELD: &str = "qualityProfileId";

const FOLDER_FIELD: &str = "rootFolderPath";

const ID_FIELD: &str = "id";

const NAMED_BY: [&str; 3] = ["title", "name", "artistName"];

fn named(item: &Map<String, Value>) -> String {
    NAMED_BY
        .iter()
        .find_map(|key| item.get(*key).and_then(Value::as_str))
        .unwrap_or_default()
        .to_owned()
}

#[async_trait]
impl<M: Music> Carrying for Curator<M> {
    async fn records(&self, kind: Record) -> Result<Vec<Carried>, Failure> {
        let kind = self.holding(kind)?;
        let held: Vec<Value> = self
            .servarr
            .read(kind.path(), &format!("the {} it holds", kind.plural()))
            .await?;
        let profiles = self.numbered_profiles().await?;
        Ok(held
            .into_iter()
            .filter_map(|item| match item {
                Value::Object(item) => Some(item),
                _ => None,
            })
            .map(|item| Carried {
                name: named(&item),
                profile: item
                    .get(PROFILE_FIELD)
                    .and_then(Value::as_u64)
                    .and_then(|id| profiles.get(&id).cloned()),
                folder: item
                    .get(FOLDER_FIELD)
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                rest: Value::Object(item).to_string(),
            })
            .filter(|carried| !carried.name.is_empty())
            .collect())
    }

    async fn carry(&self, kind: Record, item: &Carried) -> Result<(), Failure> {
        let kind = self.holding(kind)?;
        let Ok(Value::Object(mut body)) = serde_json::from_str::<Value>(&item.rest) else {
            return Err(self.servarr.endpoint().refused(&format!(
                "a carried record of {} could not be read back",
                kind.plural()
            )));
        };
        for field in [ID_FIELD, PROFILE_FIELD, FOLDER_FIELD] {
            body.remove(field);
        }
        if let Some(wanted) = &item.profile {
            let id = self.profile_named(wanted).await?;
            body.insert(PROFILE_FIELD.to_owned(), Value::from(id));
        }
        if let Some(folder) = &item.folder {
            body.insert(FOLDER_FIELD.to_owned(), Value::from(folder.clone()));
        }
        self.servarr
            .done(
                Method::Post,
                kind.path(),
                Some(Value::Object(body).to_string()),
            )
            .await
    }
}

impl<M: Music> Curator<M> {
    async fn numbered_profiles(&self) -> Result<BTreeMap<u64, String>, Failure> {
        let held: Vec<ProfileResource> = self
            .servarr
            .read(QUALITY_PROFILES, "the quality profiles")
            .await?;
        Ok(held
            .into_iter()
            .filter(|profile| !profile.name.is_empty())
            .filter_map(|profile| u64::try_from(profile.id).ok().map(|id| (id, profile.name)))
            .collect())
    }

    async fn profile_named(&self, wanted: &str) -> Result<u64, Failure> {
        self.numbered_profiles()
            .await?
            .into_iter()
            .find(|(_, name)| name == wanted)
            .map(|(id, _)| id)
            .ok_or_else(|| {
                self.servarr
                    .endpoint()
                    .refused(&format!("no quality profile called {wanted}"))
            })
    }
}

#[cfg(test)]
mod tests;
