use lemonfiber_ports::media::Kind;

pub(crate) trait Shape: Sized {
    fn upgrade_command(self) -> &'static str;

    fn release_id_param(self) -> &'static str;

    fn library_endpoint(self) -> &'static str;

    fn history_filter(self) -> &'static str;

    fn parts(self) -> Option<(&'static str, &'static str)>;

    fn reference_field(self) -> &'static str;

    fn search_option(self) -> &'static str;

    fn only_field(self) -> (&'static str, serde_json::Value);
}

impl Shape for Kind {
    fn upgrade_command(self) -> &'static str {
        match self {
            Kind::Tv => "CutoffUnmetEpisodeSearch",
            Kind::Movies => "CutoffUnmetMoviesSearch",
        }
    }

    fn release_id_param(self) -> &'static str {
        match self {
            Kind::Tv => "episodeId",
            Kind::Movies => "movieId",
        }
    }

    fn library_endpoint(self) -> &'static str {
        match self {
            Kind::Tv => "series",
            Kind::Movies => "movie",
        }
    }

    fn history_filter(self) -> &'static str {
        match self {
            Kind::Tv => "seriesIds",
            Kind::Movies => "movieIds",
        }
    }

    fn parts(self) -> Option<(&'static str, &'static str)> {
        match self {
            Kind::Tv => Some(("episode", "seriesId")),
            Kind::Movies => None,
        }
    }

    fn reference_field(self) -> &'static str {
        match self {
            Kind::Tv => "tvdbId",
            Kind::Movies => "tmdbId",
        }
    }

    fn search_option(self) -> &'static str {
        match self {
            Kind::Tv => "searchForMissingEpisodes",
            Kind::Movies => "searchForMovie",
        }
    }

    fn only_field(self) -> (&'static str, serde_json::Value) {
        match self {
            Kind::Tv => ("seasonFolder", serde_json::Value::Bool(true)),
            Kind::Movies => (
                "minimumAvailability",
                serde_json::Value::String("released".to_owned()),
            ),
        }
    }
}
