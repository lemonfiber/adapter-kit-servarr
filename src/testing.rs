use std::sync::Arc;

use lemonfiber_fixtures::http::Fake;
use lemonfiber_ports::http::Request;

use crate::client::Servarr;
use crate::curator::{Curator, Files, NoMusic};

pub(crate) const BASE: &str = "http://sonarr:8989";

pub(crate) const KEY: &str = "the-key";

pub(crate) const SERVICE: &str = "sonarr";

pub(crate) fn servarr(fake: &Arc<Fake>) -> Servarr {
    Servarr::new(fake.clone(), BASE, Arc::new(KEY.to_owned()), SERVICE, 3)
}

pub(crate) fn curator(fake: &Arc<Fake>, files: Files) -> Curator {
    Curator::new(servarr(fake), files, NoMusic)
}

pub(crate) fn television(fake: &Arc<Fake>) -> Curator {
    curator(fake, Files::TELEVISION)
}

pub(crate) fn films(fake: &Arc<Fake>) -> Curator {
    curator(fake, Files::FILMS)
}

pub(crate) fn music(fake: &Arc<Fake>) -> Curator {
    curator(fake, Files::MUSIC)
}

pub(crate) fn asked(fake: &Fake) -> Vec<String> {
    fake.requests().into_iter().map(|one| one.url).collect()
}

pub(crate) fn api(path: &str) -> String {
    format!("{BASE}/api/v3{path}")
}

pub(crate) fn sent_body(request: Option<Request>) -> serde_json::Value {
    request
        .and_then(|request| request.body)
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_default()
}

pub(crate) fn at(value: &serde_json::Value, pointer: &str) -> serde_json::Value {
    value.pointer(pointer).cloned().unwrap_or_default()
}
