//! The client lemonfiber's Sonarr, Radarr, Lidarr and Prowlarr adapters share: the
//! keyed, versioned Servarr client, and `library.curate` and `indexer.pull` as Sonarr,
//! Radarr and Lidarr answer them.

pub mod client;
pub mod credential;
pub mod curator;

#[cfg(test)]
mod testing;
