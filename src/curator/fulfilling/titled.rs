use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{CuratedSeason, CuratedTitle};
use serde::Deserialize;

use super::super::filed::Filed;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TitleResource {
    #[serde(default)]
    pub(super) id: i64,
    #[serde(default)]
    pub(super) title: String,
    #[serde(default)]
    year: Option<u32>,
    #[serde(default)]
    tvdb_id: Option<i64>,
    #[serde(default)]
    tmdb_id: Option<i64>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    monitored: bool,
    #[serde(default)]
    has_file: bool,
    #[serde(default)]
    statistics: Option<Statistics>,
    #[serde(default)]
    seasons: Vec<SeasonResource>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Statistics {
    #[serde(default, rename = "episodeFileCount")]
    episode_files: u32,
    #[serde(default, rename = "totalEpisodeCount")]
    episodes: u32,
    #[serde(default, rename = "trackFileCount")]
    track_files: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SeasonResource {
    pub(super) season_number: u32,
    #[serde(default)]
    monitored: bool,
    #[serde(default)]
    statistics: Option<Statistics>,
}

impl TitleResource {
    pub(super) const fn reference(&self, filed: Filed) -> Option<i64> {
        match filed {
            Filed::Video(Kind::Tv) => self.tvdb_id,
            Filed::Video(Kind::Movies) => self.tmdb_id,
            Filed::Album => None,
        }
    }

    pub(super) fn season_numbers(&self) -> impl Iterator<Item = u32> + '_ {
        self.seasons.iter().map(|season| season.season_number)
    }

    pub(super) fn curated(self, shape: Filed) -> CuratedTitle {
        let held = self.id != 0;
        let reference = self.reference(shape).unwrap_or_default();
        let year = shape.year(self.year, self.release_date.as_deref());
        let (files, seasons) = match shape {
            Filed::Video(Kind::Tv) => (
                self.statistics.unwrap_or_default().episode_files,
                self.seasons
                    .into_iter()
                    .map(|season| season.curated(held))
                    .collect(),
            ),
            Filed::Video(Kind::Movies) => (u32::from(self.has_file), Vec::new()),
            Filed::Album => (self.statistics.unwrap_or_default().track_files, Vec::new()),
        };
        CuratedTitle {
            reference,
            id: held.then_some(self.id),
            title: self.title,
            year: year.filter(|year| *year > 0),
            monitored: held && self.monitored,
            files,
            seasons,
        }
    }
}

impl SeasonResource {
    fn curated(self, held: bool) -> CuratedSeason {
        let statistics = self.statistics.unwrap_or_default();
        CuratedSeason {
            number: self.season_number,
            monitored: held && self.monitored,
            files: statistics.episode_files,
            episodes: statistics.episodes,
        }
    }
}
