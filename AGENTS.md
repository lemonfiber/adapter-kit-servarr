# AGENTS.md — adapter-kit-servarr

> **Start at the roadmap and board on [lemonfiber.app](https://lemonfiber.app),
> rendered from the report of where every unreleased version stands. Then the
> rules** every repository shares:
> [working in the repositories](https://github.com/lemonfiber/spec/blob/main/50-governance/working-in-the-repositories.md)
> and [the rules for agents](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> This file holds only what is true of this repository.

## What this repo is

The client the adapters in front of Sonarr, Radarr, Lidarr and Prowlarr share,
as one Rust library crate. Each adapter takes it as a git dependency at an exact
`rev`. It serves no contract, builds no image and has no binary
([spec](https://github.com/lemonfiber/spec/blob/main/30-repos/adapter-kit-servarr.md)).

## The rules you cannot break

- **Only what the Servarr adapters share** (`REPO-R99`). What one upstream alone
  answers stays in that upstream's adapter.
- **No adapter's release, digest, settings, recorder or recordings** (`REPO-R101`).
- **The core's crates move with the train.** Every `lemonfiber-*` crate in
  `Cargo.toml` names one core commit, and an adapter taking this kit names the
  same one (`REPO-R100`).
- **No `unwrap`, `expect`, `panic` or indexing** outside tests. The lints in
  `Cargo.toml` are the core's.
- **No credential in a refusal.** What an upstream says is withheld before it is
  carried into a failure, and only its own words are taken from it.
- **Every line is covered** (`REPO-R102`), over a fake HTTP transport.

## Checks

```
just ci
```
