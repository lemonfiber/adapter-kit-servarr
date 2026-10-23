# adapter-kit-servarr

**The client lemonfiber's Servarr adapters share.**
Sonarr, Radarr, Lidarr and Prowlarr speak one HTTP API. This crate speaks it once:
the keyed, versioned client, and `library.curate` as Sonarr, Radarr and Lidarr
answer it.

## How an adapter takes it

```toml
[dependencies]
adapter-kit-servarr = { git = "https://github.com/lemonfiber/adapter-kit-servarr", rev = "<commit>" }
```

The adapter names the same core commit for its `lemonfiber-*` crates as this
crate's `Cargo.toml` does. Nothing here is published to a registry.

## Checks

The crate is held to its format, its lints, its tests, full line coverage and its
dependency policy. `just ci` runs them.

## Licence

The code in this repository is under the licence in `LICENSE`.
