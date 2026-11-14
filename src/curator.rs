//! `library.curate` as Sonarr, Radarr and Lidarr answer it.

use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::{Format, Kind, TitleKind};
use lemonfiber_ports::service::{
    CutoffUnmet, Failure, Heard, Maintenance, MusicQuality, Record, Recounting, RootFolder,
};

use crate::client::Servarr;

mod carrying;
mod catalogue;
mod clients;
mod cutoff;
mod filed;
mod fulfilling;
mod grabs;
mod importing;
mod kind;
mod pipeline;
mod queue;
mod releases;
mod wire;

use filed::Filed;

const ARTIST_ID: &str = "artistId";

const COMMAND: &str = "/command";

const MONITORED: &str = "monitored";

const THE_CATALOGUE: &str = "the catalogue";

const THE_EPISODES: &str = "the episodes";

const THE_HISTORY: &str = "the history";

const THE_LIBRARY: &str = "the library";

const THE_QUALITY_PROFILES: &str = "the quality profiles";

const THE_RELEASE_SEARCH: &str = "the release search";

const THE_ROOT_FOLDERS: &str = "the root folders";

const THE_WANTED_LIST: &str = "the wanted list";

const QUALITY_PROFILES: &str = "/qualityprofile";

const RELEASE: &str = "/release";

const ROOT_FOLDERS: &str = "/rootfolder";

const WANTED: &str = "/wanted/missing?page=1&pageSize=1";

fn newest_history(size: usize) -> String {
    format!("/history?page=1&pageSize={size}&sortKey=date&sortDirection=descending")
}

/// What one curator files: the kind of title it is asked about and the records it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Files {
    /// The kind of title it files.
    pub kind: TitleKind,
    /// The records it holds.
    pub record: Record,
}

impl Files {
    /// What a curator of television files.
    pub const TELEVISION: Self = Self {
        kind: TitleKind::Tv,
        record: Record::Series,
    };

    /// What a curator of films files.
    pub const FILMS: Self = Self {
        kind: TitleKind::Movies,
        record: Record::Film,
    };

    /// What a curator of music files.
    pub const MUSIC: Self = Self {
        kind: TitleKind::Music,
        record: Record::Artist,
    };
}

/// What a curator of music answers that the others do not.
#[async_trait]
pub trait Music: Send + Sync {
    /// Apply `format` to every quality profile the curator holds.
    ///
    /// # Errors
    ///
    /// Why the format was not applied; by default, that the curator files no music.
    async fn apply(&self, servarr: &Servarr, _format: Format) -> Result<(), Failure> {
        Err(servarr.unsupported(FILES_NO_MUSIC))
    }

    /// The fields `folder` is registered with beside its path.
    ///
    /// # Errors
    ///
    /// Why the fields could not be worked out; by default there are none.
    async fn root_folder(
        &self,
        _servarr: &Servarr,
        _folder: &RootFolder,
    ) -> Result<serde_json::Map<String, serde_json::Value>, Failure> {
        Ok(serde_json::Map::new())
    }
}

const FILES_NO_MUSIC: &str = "it files no music";

const TELLS_NOTHING: &str = "this adapter does not hear what its upstream did yet";

/// A curator of no music.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMusic;

impl Music for NoMusic {}

/// `library.curate`, answered by one Servarr curator through `servarr`.
pub struct Curator<M: Music = NoMusic> {
    servarr: Servarr,
    files: Files,
    music: M,
}

impl<M: Music> Curator<M> {
    /// The curator `servarr` speaks to, filing `files`, with `music` answering what only
    /// a curator of music answers.
    pub fn new(servarr: Servarr, files: Files, music: M) -> Self {
        Self {
            servarr,
            files,
            music,
        }
    }

    /// The client every call is asked through.
    #[must_use]
    pub fn servarr(&self) -> &Servarr {
        &self.servarr
    }

    fn filing(&self, kind: Kind) -> Result<Kind, Failure> {
        if self.files.kind == TitleKind::from(kind) {
            Ok(kind)
        } else {
            Err(self.files_no(kind.noun()))
        }
    }

    fn filed(&self, kind: TitleKind) -> Result<Filed, Failure> {
        Filed::of(kind)
            .filter(|_| self.files.kind == kind)
            .ok_or_else(|| self.files_no(kind.noun()))
    }

    fn files_no(&self, noun: &str) -> Failure {
        self.servarr.unsupported(&format!("it files no {noun}"))
    }

    fn holding(&self, record: Record) -> Result<Record, Failure> {
        if self.files.record == record {
            Ok(record)
        } else {
            Err(self
                .servarr
                .unsupported(&format!("it holds no {}", record.plural())))
        }
    }
}

#[async_trait]
impl<M: Music> Maintenance for Curator<M> {
    async fn search_upgrades(&self, kind: TitleKind) -> Result<(), Failure> {
        let filed = self.filed(kind)?;
        let body = serde_json::json!({ "name": filed.upgrade_command() }).to_string();
        self.servarr.done(Method::Post, COMMAND, Some(body)).await
    }

    async fn cutoff_unmet(&self, kind: TitleKind) -> Result<CutoffUnmet, Failure> {
        cutoff::below(&self.servarr, self.filed(kind)?).await
    }

    async fn wanting(&self) -> Result<bool, Failure> {
        let wanted: Wanted = self.servarr.read(WANTED, THE_WANTED_LIST).await?;
        Ok(!wanted.records.is_empty())
    }
}

#[derive(serde::Deserialize)]
struct Wanted {
    #[serde(default)]
    records: Vec<serde_json::Value>,
}

#[async_trait]
impl<M: Music> MusicQuality for Curator<M> {
    async fn apply_music_format(&self, format: Format) -> Result<(), Failure> {
        self.music.apply(&self.servarr, format).await
    }
}

#[async_trait]
impl<M: Music> Recounting for Curator<M> {
    async fn events(
        &self,
        _address: Option<&str>,
        _pin: Option<&str>,
        _key: Option<&str>,
        _every: Option<u32>,
    ) -> Result<Option<Heard>, Failure> {
        Err(self.servarr.unsupported(TELLS_NOTHING))
    }
}

#[cfg(test)]
mod tests;
