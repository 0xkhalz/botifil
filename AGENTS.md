# botifil — Agent Instructions

Telegram join-request gatekeeper bot (`Rust 1.99`, edition `2024`, `teloxide 0.17`, embedded `redb 5.0` session store, optional Tokio HTTP/1.1 Telegram Mini App).
Keep this file synchronized whenever runtime behavior, env vars, wire/storage schemas, or invariants change.

## Quick Commands & Verification Pipeline
- **Working directory & setup**: repo root; `mise install` (`mise.toml`), then `cp .env.example .env` (set `TELOXIDE_TOKEN`).
- **Fast inner loop**:
  - Warm single test (`~0.5s`): `mise exec -- cargo nextest run --locked -E 'test(test_domain)'`
  - Warm full test suite (`~6s`; cold rebuild `~65s` with `CARGO_BUILD_JOBS=2`): `mise exec -- cargo nextest run --locked`
  - Fast docs/security lint: `./scripts/verify.sh --docs`
- **Full verification sequence** (MUST exit `0` ending in `verify: OK` before opening/updating a PR):
```bash
./scripts/verify.sh        # Full gate: --docs + --rust (matches .github/workflows/ci.yml)
./scripts/verify.sh --docs # typos, lychee --offline --include-fragments, actionlint, gitleaks (dir + git)
./scripts/verify.sh --rust # fmt --check, clippy --all-targets --locked (-D warnings), nextest, doc tests, cargo-deny, cargo-shear
```
- **Runner & sandbox caveats**:
  - Low-RAM host defaults: `CARGO_BUILD_JOBS=2` (`mise.toml`); `[profile.dev]` sets `codegen-units = 1`, `debug = "line-tables-only"`, `incremental = false` (`Cargo.toml`). AVOID concurrent Cargo builds.
  - `gitleaks dir .` (`--docs`) scans the working directory including untracked `.env` and `target/`; run `./scripts/verify.sh --docs` in a clean worktree without local `.env` or build artifacts.
  - `cargo shear` fails on unused dependencies; `cargo deny check` enforces allowed licenses, no wildcard versions, and advisory checks (`deny.toml`).
  - NEVER run `./scripts/dev-tunnel.sh` (spawns live `cloudflared` + `cargo run --locked`) or `./scripts/gh-bootstrap.sh` (mutates GitHub labels and branch rulesets via `gh api`) during routine verification.
  - Container builds (`Containerfile`, `compose.yaml`): multi-stage static `x86_64-unknown-linux-musl` (`musl-tools` required for `ring` C compilation) on `gcr.io/distroless/static-debian12:nonroot` (`65532:65532`), `read_only: true`, `cap_drop: [ALL]`, `no-new-privileges:true`, `./data:/data` bind mount, `cloudflared` sidecar (`CLOUDFLARE_TUNNEL_TOKEN`). Rootless Podman requires `podman unshare chown 65532:65532 ./data`.

## Architecture Map & Task Routing
- **Domain, Errors & Screening**: `src/domain.rs`, `src/error.rs`, `src/screening.rs` — `SessionToken`, `Challenge`, `CallbackAction`, `PendingJoin`, `SessionOutcome`, `BotErrorAction`, bio substring screener. **Must not own**: I/O, storage, network calls, or regex/external crates.
- **Config**: `src/config.rs` (`src/config_tests.rs`) — env parsing/validation (`Config::from_env`) and redacted `Debug`. **Must not own**: runtime state or secret logging.
- **Persistence**: `src/store/` (`mod.rs`, `memory.rs`, `redb.rs`, `reconciliation.rs`) — `Storage` trait, `MemoryStore`, `RedbSessionStore`, startup expiry reconciliation. **Must not own**: Telegram API calls or blocking `redb` I/O on Tokio async worker threads.
- **Telegram Handlers**: `src/handlers/` (`mod.rs`, `join_request.rs`, `callback.rs`) — `dptree` schema (`ChatJoinRequest`, `CallbackQuery`, `ChatMember`), challenge dispatch, inline callback handling. **Must not own**: duplicate approve/decline resolution logic (MUST delegate to `webapp::resolve_join_request`).
- **WebApp & Shared Resolver**: `src/webapp/` (`mod.rs`, `auth.rs`, `server.rs`, `ui.html`) — shared `resolve_join_request`, pure-Rust SHA-256/HMAC-SHA256 `initData` verifier, raw Tokio HTTP/1.1 server, embedded Rose-Pine Mini App (`INDEX_HTML`). **Must not own**: web framework or crypto crates, or leaking `chat_id`/bot token in HTTP responses.
- **Process Bootstrap**: `src/main.rs` — `.env` + `Config::from_env`, `spawn_blocking(SessionStore::open_redb)`, `reconcile_on_startup`, `Bot::throttle(Limits::default())`, `getMe`, `Dispatcher`, 5s `run_expiry_reaper`, conditional WebApp server (ONLY when `BOTIFIL_WEBAPP_URL` is set), and `SIGINT`/`SIGTERM` `watch` shutdown.
- **If changing join-resolution ordering, DM edits, or idempotent error handling**: inspect `src/webapp/mod.rs` (`resolve_join_request`) AND both callers (`src/handlers/callback.rs`, `src/webapp/server.rs`) plus `tests/gatekeeper_e2e.rs` and `tests/webapp_e2e.rs`.
- **If changing `PendingJoin` fields or persistence format**: inspect `src/store/redb.rs` (`[u8; 44]` row codec) AND `tests/persistence_test.rs`.
- **If changing WebApp routes, subpath handling, or JSON responses**: inspect `src/webapp/server.rs` AND `src/webapp/ui.html` (uses relative `./api/challenge` and `./api/verify`) plus `tests/webapp_e2e.rs`.
- **If changing env vars, defaults, or validation**: inspect `src/config.rs`, `src/config_tests.rs`, [.env.example](.env.example), `compose.yaml`, [README.md](README.md), and `scripts/dev-tunnel.sh` (parses `BOTIFIL_WEBAPP_BIND` from `.env`).
- **If changing dependencies or advisories**: inspect `Cargo.toml`, `deny.toml`, and `Containerfile` (static musl build).

## Repo-Specific Conventions & Domain Patterns
- **Compiler & lint gates (`Cargo.toml`, `rustfmt.toml`)**: `unsafe_code = "forbid"`, `clippy::pedantic = "warn"`, `unwrap_used` / `expect_used` / `panic = "deny"` outside tests, `max_width = 100`.
- **Error & logging crates**: ONLY `thiserror` in library modules; `anyhow` ONLY in `src/main.rs`; `tracing` for structured logs (NEVER `println!`/`eprintln!`).
- **Zero-framework / minimal-dep rule**: KEEP raw Tokio HTTP/1.1 (`src/webapp/server.rs`), pure-Rust HMAC-SHA256 (`src/webapp/auth.rs`), and case-insensitive substring bio screening (`src/screening.rs`). NEVER add crates or new `deny.toml` advisory ignores without explicit maintainer approval (existing ignores: `RUSTSEC-2026-0173`, `RUSTSEC-2026-0543` transitive via `teloxide 0.17`).
- **Config & privacy redaction**: `Config::Debug` redacts `telegram_token` and `db_path` (`[REDACTED]`) and logs only `allowed_chats_count`. NEVER read `.env`/`.env.local` (inspect [.env.example](.env.example)), and NEVER log or commit tokens, chat/user IDs, or personal paths.
- **Config parsing edge cases (`src/config.rs`)**: unset `BOTIFIL_WEBAPP_URL` falls back to inline buttons, whereas empty/whitespace `BOTIFIL_WEBAPP_URL=""` or `BOTIFIL_DB_PATH=""` errors; `BOTIFIL_WEBAPP_URL` requires `https://` (or `http://` on `localhost`/`127.0.0.1`/`::1`) with no userinfo; `BOTIFIL_CHALLENGE_TTL_SECS` is `1..=300` (default `300`), `BOTIFIL_MAX_ATTEMPTS` is `1..=5` (default `2`), empty `BOTIFIL_ALLOWED_CHATS` allows all chats.
- **Repo branch & docs gates**: `main` ruleset ([.github/rulesets/main.json](.github/rulesets/main.json)) requires PRs, linear history (`rebase`-only merge), and passing `ci-ok` + `pr-title` ([.github/workflows/pr-title.yml](.github/workflows/pr-title.yml): `feat|fix|docs|chore|refactor|test|ci|perf`); include the `./scripts/verify.sh` `verify: OK` tail per [.github/PULL_REQUEST_TEMPLATE.md](.github/PULL_REQUEST_TEMPLATE.md). ONLY three root docs exist ([README.md](README.md), `AGENTS.md`, [CHANGELOG.md](CHANGELOG.md)); NEVER create extra root docs or `docs/` trees.

## Non-Negotiable Invariants & Footguns
- **Three distinct Telegram IDs (NEVER swap)**:
  - `req.chat.id` (`ChatId`) -> target group/channel (`approveChatJoinRequest` / `declineChatJoinRequest`).
  - `req.from.id` (`UserId`) -> applicant (`approveChatJoinRequest` / `declineChatJoinRequest`).
  - `req.user_chat_id` (`ChatId`) -> ephemeral DM target (`sendMessage` / `editMessageText` ONLY).
- **300s DM ticket & resolve-before-feedback ordering**:
  - A join request grants a 300s DM ticket without `/start`; admin dismissal emits NO update and closes the ticket.
  - ALWAYS persist via `store.upsert` BEFORE `sendMessage` (on `Ok(msg)` call `store.set_message_id`; on `Err` call `store.remove_by_token`).
  - ALWAYS call `approveChatJoinRequest` / `declineChatJoinRequest` BEFORE `answerCallbackQuery` and best-effort `editMessageText` so admin-dismissed requests never show false approval.
  - `chat_member` updates (`src/handlers/mod.rs`) MUST evict stale sessions via `store.remove_by_peer(chat_id, user_id)`.
- **Leave-pending vs auto-decline policy**:
  - Auto-decline ONLY on bio screening matches (`t.me/`, `telegram.me/`, `tg://`, `http://`, `https://`, case-insensitive) or exhausted challenge attempts (`BOTIFIL_MAX_ATTEMPTS`).
  - Leave pending (NEVER auto-decline) on `403 BotBlocked` / `USER_IS_BLOCKED` / `CantInitiateConversation`, TTL expiry (5s reaper or startup reconciliation), or `StoreError::CapacityExceeded` (`DEFAULT_MAX_CAPACITY = 4096`).
- **Telegram limits & error classification (`src/error.rs`)**:
  - Bot requires `can_invite_users`; `creates_join_request = true` is mutually exclusive with `member_limit` (`400 MEMBER_LIMIT_INVALID`).
  - Callback data `<= 64` UTF-8 bytes (`MAX_CALLBACK_DATA_BYTES`, format `v1:<token>:<answer>`; `SessionToken` is 16 hex chars, validated `1..=32` ASCII alphanumeric; `Challenge` is `1..=9 + 1..=9` with 3 distinct choices).
  - `BOTIFIL_WEBAPP_URL` DM button (`InlineKeyboardButton::web_app`) appends `?token=<token>` while preserving existing subpath and query pairs.
  - Map `400 HIDE_REQUESTER_MISSING` -> `IdempotentNoOp`, `400 USER_ALREADY_PARTICIPANT` -> `IdempotentSuccess`, `400 CHAT_ADMIN_REQUIRED` / `RIGHTS_INSUFFICIENT` -> `FatalAdminRequired`, `400 USER_ID_INVALID` -> `BugInvalidUserId`, `429 RetryAfter` -> `RateLimited` (handled by `Bot::throttle`).
- **Persistence & binary schema lock (`src/store/`)**:
  - `Storage::is_blocking()` MUST return `true` for `RedbSessionStore` (`false` for `MemoryStore`) so `SessionStore` offloads all `redb` I/O via `tokio::task::spawn_blocking`.
  - Table `"pending_joins"` maps `&str` -> fixed 44-byte little-endian `[u8; 44]`: `chat_id[0..8]`, `user_id[8..16]`, `user_chat_id[16..24]`, `left/right/c0/c1/c2/attempts/max_attempts[24..31]`, `msg_tag[31]` (`0`=None, `1`=Some), `msg_id[32..36]`, `expires_at_ms[36..44]`. Changing byte layout breaks existing `.redb` files.
  - `RedbSessionStore::open` auto-removes and recreates the ephemeral DB on `redb::DatabaseError::UpgradeRequired`, while preserving non-`redb` files untouched and returning `StoreError::Backend`.
  - `Instant` <-> Unix epoch ms uses startup `(anchor, anchor_ms)`.
- **WebApp HTTP & Auth contract (`src/webapp/`)**:
  - Routes (after stripping base path from `BOTIFIL_WEBAPP_URL`): `GET /` (`INDEX_HTML`), `GET /healthz` (`200 {"status":"ok"}`), `GET /api/challenge?token=<tok>` (non-mutating `get_by_token`), `POST /api/verify` (`application/x-www-form-urlencoded`: `token`, `answer`, `init_data`).
  - Hardening (`server.rs`): `MAX_HEADER_BYTES = 8192`, `MAX_BODY_BYTES = 16_384`, `READ_TIMEOUT = 5s`; rejects `Transfer-Encoding` and duplicate/non-digit `Content-Length`; ALWAYS sets `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Connection: close`; NEVER leaks `chat_id` or bot token in responses.
  - Auth (`auth.rs`): `HMAC_SHA256("WebAppData", bot_token)` over sorted `k=v\n` check string with constant-time compare (`std::hint::black_box`), rejects duplicate `hash`, enforces `auth_date` within `challenge_ttl` (+60s future skew tolerance), extracts top-level numeric `user.id`, and binds it to `PendingJoin.user_id` (`WrongUser` -> `403`).
- **Known failure signatures**:
  - `400 USER_ID_INVALID` -> swapped `chat_id` / `user_id` / `user_chat_id` bug (`BugInvalidUserId`); inspect ID arguments first.
  - `400 MEMBER_LIMIT_INVALID` -> invite link created with both `creates_join_request = true` and `member_limit`.
  - Telegram Mini App stuck/blocked behind Cloudflare -> bypass Bot Fight Mode and JS/Turnstile challenges for the WebApp route (in-app WebView cannot solve browser challenges).
  - `cargo deny check` failing on `proc-macro-error2` / `chrono` advisories -> inspect `deny.toml` transitive ignores before touching product code.

## Testing & Verification Notes
- **Layout & targeted invocation**:
  - Unit tests live inline (`#[cfg(test)]`) or in sibling files (`src/config_tests.rs`, `src/store/memory_tests.rs`); integration suites live in `tests/` (`gatekeeper_e2e.rs`, `persistence_test.rs`, `webapp_e2e.rs`).
  - Run one integration suite: `mise exec -- cargo nextest run --locked --test webapp_e2e` (also `gatekeeper_e2e`, `persistence_test`).
  - Run one filtered test: `mise exec -- cargo nextest run --locked -E 'test(test_domain)'`.
  - Run doc tests only: `mise exec -- cargo test --doc --locked`.
- **Mocking & isolation rules**:
  - NEVER hit the live Telegram API in tests; mock via `wiremock::MockServer` + `Bot::new("test_token").set_api_url(...)` and `MemoryStore` or temp-dir `redb` (`TempDb` / `Tmp` RAII cleanup).
  - Use `botifil::webapp::auth::sign_init_data_for_test` to generate signed `initData` payloads in WebApp tests.

## Progressive Disclosure
Read only the relevant section on demand:
- [README.md](README.md) — quickstart, env var reference table, Cloudflare Tunnel & rootless Podman deployment, security reporting.
- [CHANGELOG.md](CHANGELOG.md) — unreleased changes and rationale for key architectural decisions.
- [.env.example](.env.example) — canonical environment variable names and defaults.
- [.github/workflows/ci.yml](.github/workflows/ci.yml), [.github/rulesets/main.json](.github/rulesets/main.json) — authoritative CI jobs and `main` branch protection rules.
