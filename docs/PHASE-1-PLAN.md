# Phase 1: Workspace Skeleton, Boundaries & Offline Source Plan

## Goal
Implement strict architecture boundaries across the workspace:
1. `yt-model`: Pure domain entities (`VideoCard`, `VideoDetails`, `ChannelPage`, `PlayableMedia`, etc.) with `serde` support.
2. `yt-source`: Generic async `StreamSource` trait + offline fixture-backed mock source (`FixtureSource`).
3. `store`: Thread-safe SQLite persistence with WAL mode, `PRAGMA synchronous = NORMAL`, and versioned migrations (`PRAGMA user_version`).
4. `src-tauri`: IPC security guard enforcing window label permissions, minimal capability scopes, and tracing with secret redaction.
5. All workspace units compiling and testable offline (`cargo test --workspace` all green).

## Tasks
- [ ] Task 1.1: Define domain types in `yt-model` (`VideoCard`, `VideoDetails`, `StreamFormat`, `QualityPref`, `Capabilities`).
- [ ] Task 1.2: Implement `StreamSource` async trait in `yt-source` + `FixtureSource` parsing JSON fixtures from `tests/fixtures/`.
- [ ] Task 1.3: Implement `store` database connection pool, user settings table, watch history table, and schema migrations.
- [ ] Task 1.4: Implement IPC label security guard and command routing in `src-tauri`.
- [ ] Task 1.5: Verify offline compilation and unit tests with `cargo test --workspace`.
