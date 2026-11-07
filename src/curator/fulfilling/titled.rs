use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{CuratedSeason, CuratedTitle};
use serde::Deserialize;

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
    #[serde(default)]
    episode_file_count: u32,
    #[serde(default)]
    total_episode_count: u32,
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
    pub(super) fn reference(&self, kind: Kind) -> Option<i64> {
        match kind {
            Kind::Tv => self.tvdb_id,
            Kind::Movies => self.tmdb_id,
        }
    }

    pub(super) fn season_numbers(&self) -> impl Iterator<Item = u32> + '_ {
        self.seasons.iter().map(|season| season.season_number)
    }

    pub(super) fn curated(self, kind: Kind) -> CuratedTitle {
        let held = self.id != 0;
        let reference = self.reference(kind).unwrap_or_default();
        let (files, seasons) = match kind {
            Kind::Tv => (
                self.statistics.unwrap_or_default().episode_file_count,
                self.seasons
                    .into_iter()
                    .map(|season| season.curated(held))
                    .collect(),
            ),
            Kind::Movies => (u32::from(self.has_file), Vec::new()),
        };
        CuratedTitle {
            reference,
            id: held.then_some(self.id),
            title: self.title,
            year: self.year.filter(|year| *year > 0),
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
            files: statistics.episode_file_count,
            episodes: statistics.total_episode_count,
        }
    }
}
