# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Join-request gatekeeper**: Filters incoming `chat_join_request` updates by `BOTIFIL_ALLOWED_CHATS`, screens applicant bios for spam URLs and invite links, and sends an arithmetic DM captcha to `user_chat_id` with configurable TTL (`BOTIFIL_CHALLENGE_TTL_SECS`) and retry limit (`BOTIFIL_MAX_ATTEMPTS`).
- **Inline callback & `chat_member` handling**: Resolves join requests from inline DM buttons, handles `400 HIDE_REQUESTER_MISSING` and `400 USER_ALREADY_PARTICIPANT` idempotently, leaves requests pending on `403` or TTL expiry, and evicts sessions on `chat_member` changes.
- **Persistent `redb` session store**: Added `Storage` trait, `RedbSessionStore` (`BOTIFIL_DB_PATH`), startup expiry reconciliation, and a 5-second background reaper alongside `MemoryStore`.
- **Optional Telegram WebApp verification**: Added an embedded Mini App served over HTTP/1.1 (`BOTIFIL_WEBAPP_URL`, `BOTIFIL_WEBAPP_BIND`) with `/`, `/api/verify`, and subpath-aware `/healthz` endpoints, plus `./scripts/dev-tunnel.sh` for local `cloudflared` quick tunnels.
- **Container deployment & CI**: Added static `x86_64-unknown-linux-musl` `Containerfile` on distroless nonroot, `compose.yaml` with `cloudflared` sidecar (`CLOUDFLARE_TUNNEL_TOKEN`), `./scripts/verify.sh`, and `wiremock` E2E test suites (`gatekeeper_e2e`, `persistence_test`, `webapp_e2e`).

### Changed

- **Upgrade `redb` to 4.6 & legacy store recreation**: Bumped `redb` from 2.6 to 4.6 and updated `RedbSessionStore::open` to recreate ephemeral session store files when an incompatible older `redb` file format is detected on startup.
- **Resolve-before-feedback ordering**: Calls `approveChatJoinRequest` / `declineChatJoinRequest` before `answerCallbackQuery` and best-effort `editMessageText` so admin-dismissed requests cannot show false approvals.
- **Single-source-of-truth documentation**: Consolidated project documentation into [README.md](README.md), [AGENTS.md](AGENTS.md), and [CHANGELOG.md](CHANGELOG.md).

### Architecture

- **Rust + `teloxide` 0.17 (`edition = "2024"`, Rust `1.99`)**: Enforces `ChatId` vs `UserId` type safety, `Bot::throttle` rate limiting, `dptree` dispatch, `unsafe_code = "forbid"`, and denied `unwrap`/`expect`/`panic!` outside tests, with low-RAM dev profile settings (`CARGO_BUILD_JOBS=2`).
- **Pure-Rust `redb` persistence**: Chosen over SQLite/PostgreSQL to avoid C toolchain dependencies in static musl builds while providing ACID MVCC durability on low-RAM hosts via `spawn_blocking`.
- **Embedded Tokio HTTP/1.1 WebApp + pure-Rust HMAC-SHA256**: Serves the Mini App and verifies Telegram `WebAppData` `initData` payloads with constant-time comparison directly in-process without adding `axum` or extra crypto crates.
- **Static musl distroless + Podman `cloudflared` sidecar**: Packages a single static musl binary in `gcr.io/distroless/static-debian12:nonroot` behind a Cloudflare Tunnel sidecar for TLS termination without exposing host ports or a container shell.
