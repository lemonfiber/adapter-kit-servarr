use std::str::FromStr;

use lemonfiber_ports::media::{Kind, TitleKind};

use super::kind::Shape as _;

const ALBUM: &str = "album";

const ALBUM_ID: &str = "albumId";

const ALBUM_SEARCH: &str = "AlbumSearch";

const WITH_VIDEO: &str = "&includeSeries=true&includeMovie=true";

const WITH_ALBUM: &str = "&includeAlbum=true";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Filed {
    Video(Kind),
    Album,
}

impl From<Kind> for Filed {
    fn from(kind: Kind) -> Self {
        Self::Video(kind)
    }
}

impl Filed {
    pub(crate) const fn of(kind: TitleKind) -> Option<Self> {
        match kind {
            TitleKind::Tv => Some(Self::Video(Kind::Tv)),
            TitleKind::Movies => Some(Self::Video(Kind::Movies)),
            TitleKind::Music => Some(Self::Album),
            TitleKind::Books => None,
        }
    }

    pub(crate) const fn noun(self) -> &'static str {
        match self {
            Self::Video(kind) => kind.noun(),
            Self::Album => TitleKind::Music.noun(),
        }
    }

    pub(crate) fn library_endpoint(self) -> &'static str {
        match self {
            Self::Video(kind) => kind.library_endpoint(),
            Self::Album => ALBUM,
        }
    }

    pub(crate) fn history_filter(self) -> &'static str {
        match self {
            Self::Video(kind) => kind.history_filter(),
            Self::Album => ALBUM_ID,
        }
    }

    pub(crate) fn blocklist_filter(self) -> Option<&'static str> {
        match self {
            Self::Video(kind) => Some(kind.history_filter()),
            Self::Album => None,
        }
    }

    pub(crate) fn owner_param(self) -> &'static str {
        match self {
            Self::Video(kind) => kind.owner_param(),
            Self::Album => ALBUM_ID,
        }
    }

    pub(crate) fn release_id_param(self) -> &'static str {
        match self {
            Self::Video(kind) => kind.release_id_param(),
            Self::Album => ALBUM_ID,
        }
    }

    pub(crate) fn parts(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Video(kind) => kind.parts(),
            Self::Album => None,
        }
    }

    pub(crate) fn title_search(self, id: i64) -> serde_json::Value {
        match self {
            Self::Video(kind) => kind.title_search(id),
            Self::Album => serde_json::json!({ "name": ALBUM_SEARCH, "albumIds": [id] }),
        }
    }

    pub(crate) fn episode_detail(self) -> &'static str {
        match self {
            Self::Video(kind) => kind.episode_detail(),
            Self::Album => "",
        }
    }

    pub(crate) const fn with_items(self) -> &'static str {
        match self {
            Self::Video(_) => WITH_VIDEO,
            Self::Album => WITH_ALBUM,
        }
    }

    pub(crate) fn year<Y: FromStr>(self, said: Option<Y>, released: Option<&str>) -> Option<Y> {
        match self {
            Self::Video(_) => said,
            Self::Album => released?.split('-').next()?.parse().ok(),
        }
    }
}

#[cfg(test)]
mod tests;
