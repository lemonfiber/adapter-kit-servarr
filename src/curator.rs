//! `library.curate` as Sonarr, Radarr and Lidarr answer it.

use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::{Format, Kind};
use lemonfiber_ports::service::{Failure, Maintenance, MusicQuality, Record, RootFolder};

use crate::client::Servarr;

mod carrying;
mod catalogue;
mod clients;
mod importing;
mod kind;
mod pipeline;
mod queue;
mod releases;
mod wire;

use kind::Shape as _;

const COMMAND: &str = "/command";

const QUALITY_PROFILES: &str = "/qualityprofile";

const ROOT_FOLDERS: &str = "/rootfolder";

fn newest_history(size: usize) -> String {
    format!("/history?page=1&pageSize={size}&sortKey=date&sortDirection=descending")
}

/// What one curator files: the kind of video it is asked about, if any, and the records
/// it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Files {
    /// The kind of video it files, or none for a curator of music.
    pub kind: Option<Kind>,
    /// The records it holds.
    pub record: Record,
}

impl Files {
    /// What a curator of television files.
    pub const TELEVISION: Self = Self {
        kind: Some(Kind::Tv),
        record: Record::Series,
    };

    /// What a curator of films files.
    pub const FILMS: Self = Self {
        kind: Some(Kind::Movies),
        record: Record::Film,
    };

    /// What a curator of music files.
    pub const MUSIC: Self = Self {
        kind: None,
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
        Err(servarr.endpoint().unsupported(FILES_NO_MUSIC))
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
        if self.files.kind == Some(kind) {
            Ok(kind)
        } else {
            Err(self
                .servarr
                .endpoint()
                .unsupported(&format!("it files no {}", kind.noun())))
        }
    }

    fn holding(&self, record: Record) -> Result<Record, Failure> {
        if self.files.record == record {
            Ok(record)
        } else {
            Err(self
                .servarr
                .endpoint()
                .unsupported(&format!("it holds no {}", record.plural())))
        }
    }
}

#[async_trait]
impl<M: Music> Maintenance for Curator<M> {
    async fn search_upgrades(&self, kind: Kind) -> Result<(), Failure> {
        let kind = self.filing(kind)?;
        let body = serde_json::json!({ "name": kind.upgrade_command() }).to_string();
        self.servarr.done(Method::Post, COMMAND, Some(body)).await
    }
}

#[async_trait]
impl<M: Music> MusicQuality for Curator<M> {
    async fn apply_music_format(&self, format: Format) -> Result<(), Failure> {
        self.music.apply(&self.servarr, format).await
    }
}

#[cfg(test)]
mod tests;
