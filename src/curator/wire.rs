use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Category, ClientProbe, RegisteredClient};
use serde::Deserialize;

use super::filed::Filed;

const CATEGORY_SUFFIX: &str = "Category";

const HOST: &str = "host";

const PORT: &str = "port";

const FAILURES_BETWEEN: &str = "; ";

#[derive(Deserialize)]
pub(crate) struct StatusMessage {
    #[serde(default)]
    pub(crate) messages: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", bound = "T: Deserialize<'de>")]
pub(crate) struct Page<T> {
    #[serde(default)]
    pub(crate) total_records: usize,
    #[serde(default)]
    pub(crate) records: Vec<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueueRecord {
    #[serde(default)]
    pub(crate) id: Option<i64>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) error_message: Option<String>,
    #[serde(default)]
    pub(crate) status_messages: Vec<StatusMessage>,
    #[serde(default)]
    pub(crate) download_id: Option<String>,
    #[serde(default)]
    pub(crate) tracked_download_status: String,
    #[serde(default)]
    pub(crate) tracked_download_state: String,
    #[serde(default)]
    pub(crate) series_id: Option<i64>,
    #[serde(default)]
    pub(crate) movie_id: Option<i64>,
    #[serde(default)]
    pub(crate) album_id: Option<i64>,
    #[serde(default)]
    pub(crate) episode_id: Option<i64>,
    #[serde(default)]
    pub(crate) series: Option<TitledResource>,
    #[serde(default)]
    pub(crate) movie: Option<TitledResource>,
    #[serde(default)]
    pub(crate) album: Option<TitledResource>,
    #[serde(default)]
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) size: Option<serde_json::Number>,
    #[serde(default)]
    pub(crate) sizeleft: Option<serde_json::Number>,
    #[serde(default)]
    pub(crate) estimated_completion_time: Option<String>,
    #[serde(default)]
    pub(crate) episode: Option<EpisodeResource>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EpisodeResource {
    #[serde(default)]
    pub(crate) season_number: u32,
    #[serde(default)]
    pub(crate) episode_number: u32,
    #[serde(default)]
    pub(crate) title: String,
}

#[derive(Deserialize)]
pub(crate) struct TitledResource {
    #[serde(default)]
    pub(crate) title: String,
}

impl QueueRecord {
    pub(crate) const fn item(&self) -> Option<i64> {
        match (self.episode_id, self.movie_id) {
            (Some(episode), _) => Some(episode),
            (None, Some(movie)) => Some(movie),
            (None, None) => self.album_id,
        }
    }

    pub(crate) fn is_for(&self, filed: Filed, id: i64) -> bool {
        self.owner(filed) == Some(id)
    }

    pub(crate) const fn owner(&self, filed: Filed) -> Option<i64> {
        match filed {
            Filed::Video(Kind::Tv) => self.series_id,
            Filed::Video(Kind::Movies) => self.movie_id,
            Filed::Album => self.album_id,
        }
    }

    pub(crate) fn item_title(&self, filed: Filed) -> Option<String> {
        match filed {
            Filed::Video(Kind::Tv) => self.series.as_ref(),
            Filed::Video(Kind::Movies) => self.movie.as_ref(),
            Filed::Album => self.album.as_ref(),
        }
        .map(|resource| resource.title.clone())
        .filter(|title| !title.is_empty())
    }
}

#[derive(Deserialize)]
pub(crate) struct ClientResource {
    pub(crate) id: i64,
    #[serde(default)]
    pub(crate) fields: Vec<ClientField>,
}

#[derive(Deserialize)]
pub(crate) struct ClientField {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) value: serde_json::Value,
}

impl ClientResource {
    pub(crate) fn endpoint(self) -> Option<RegisteredClient> {
        let host = self.field(HOST)?.as_str()?.to_owned();
        let port = u16::try_from(self.field(PORT)?.as_u64()?).ok()?;
        Some(RegisteredClient {
            id: self.id.to_string(),
            host,
            port,
            category: self.category(),
        })
    }

    fn category(&self) -> Option<Category> {
        let field = self
            .fields
            .iter()
            .find(|field| field.name.ends_with(CATEGORY_SUFFIX))?;
        Some(Category {
            field: field.name.clone(),
            value: field.value.as_str()?.to_owned(),
        })
    }

    fn field(&self, name: &str) -> Option<&serde_json::Value> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.value)
    }
}

#[derive(Deserialize)]
pub(crate) struct FolderResource {
    pub(crate) id: i64,
    pub(crate) path: String,
}

#[derive(Deserialize)]
pub(crate) struct ProfileResource {
    #[serde(default)]
    pub(crate) id: i64,
    #[serde(default)]
    pub(crate) name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestResource {
    pub(crate) id: i64,
    pub(crate) is_valid: bool,
    #[serde(default)]
    pub(crate) validation_failures: Vec<TestFailure>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestFailure {
    #[serde(default)]
    pub(crate) error_message: String,
}

impl TestResource {
    pub(crate) fn probe(self) -> ClientProbe {
        let detail = if self.is_valid {
            None
        } else {
            let joined = self
                .validation_failures
                .into_iter()
                .map(|failure| failure.error_message)
                .filter(|message| !message.is_empty())
                .collect::<Vec<_>>()
                .join(FAILURES_BETWEEN);
            (!joined.is_empty()).then(|| lemonfiber_error::withheld::withheld_text(&joined))
        };
        ClientProbe {
            id: self.id.to_string(),
            reachable: self.is_valid,
            detail,
        }
    }
}
