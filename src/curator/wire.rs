use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Category, ClientProbe, RegisteredClient};
use serde::Deserialize;

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
#[serde(rename_all = "camelCase")]
pub(crate) struct QueueResource {
    #[serde(default)]
    pub(crate) total_records: usize,
    #[serde(default)]
    pub(crate) records: Vec<QueueRecord>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueueRecord {
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
    pub(crate) episode_id: Option<i64>,
    #[serde(default)]
    pub(crate) series: Option<TitledResource>,
    #[serde(default)]
    pub(crate) movie: Option<TitledResource>,
}

#[derive(Deserialize)]
pub(crate) struct TitledResource {
    #[serde(default)]
    pub(crate) title: String,
}

impl QueueRecord {
    pub(crate) const fn item(&self) -> Option<i64> {
        match self.episode_id {
            Some(episode) => Some(episode),
            None => self.movie_id,
        }
    }

    pub(crate) fn is_for(&self, kind: Kind, id: i64) -> bool {
        let owner = match kind {
            Kind::Tv => self.series_id,
            Kind::Movies => self.movie_id,
        };
        owner == Some(id)
    }

    pub(crate) fn item_title(&self, kind: Kind) -> Option<String> {
        match kind {
            Kind::Tv => self.series.as_ref(),
            Kind::Movies => self.movie.as_ref(),
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
