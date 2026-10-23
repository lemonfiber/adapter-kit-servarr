//! The client lemonfiber's Sonarr, Radarr, Lidarr and Prowlarr adapters share: the
//! keyed, versioned Servarr client, and `library.curate` as Sonarr, Radarr and Lidarr
//! answer it.

pub mod client;
pub mod credential;
pub mod curator;

#[cfg(test)]
mod testing;
