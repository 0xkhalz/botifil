# botifil — Agent Instructions

Telegram bot that verifies and approves/declines group join requests (Rust + `teloxide`).
This file is the single technical reference for AI agents and developers; keep it accurate whenever behavior, schemas, routes, or invariants change.

## 1. Toolchain, Low-RAM Build & Verification
- **Rust**: `1.99` stable, edition `2024` (`rust-toolchain.toml`, `Cargo.toml`); CLI tools installed via `mise install` (`mise.toml`).
- **Low-RAM constraints**: `CARGO_BUILD_JOBS=2` (`mise.toml`), dev profile sets `codegen-units = 1`, `debug = "line-tables-only"`, `incremental = false`. Avoid parallel heavy builds.
- **Verification gate** (must exit `0` ending in `verify: OK` before opening/updating a PR):
  ```bash
  ./scripts/verify.sh        # Full gate: docs + rust
  ./scripts/verify.sh --docs # typos, lychee --offline, actionlint, gitleaks (dir + git)
  ./scripts/verify.sh --rust # fmt, clippy (-D warnings), nextest, doc tests, cargo-deny, cargo-shear
  ```

## 2. Runtime Lifecycle & Data Flow
1. **Startup (`src/main.rs`)**: loads `.env` + `Config::from_env()`, opens `redb` via `spawn_blocking(SessionStore::open_redb)`, runs `reconcile_on_startup` (evicts expired sessions without declining them), wraps `Bot` in `Throttle` (`Limits::default()`), verifies token with `getMe`, then starts:
   - `teloxide` `Dispatcher` (`handlers::schema::<Throttle<Bot>>()`) with polling `allowed_updates = [ChatJoinRequest, CallbackQuery, ChatMember]`.
   - 5-second expiry reaper (`run_expiry_reaper`) evicting expired sessions while leaving Telegram requests pending.
   - Optional embedded WebApp HTTP/1.1 server (`webapp::server::serve`) **only** when `BOTIFIL_WEBAPP_URL` is set.
   - Graceful `SIGINT`/`SIGTERM` shutdown via `watch::channel(false)` and `dp.shutdown_token()`.
2. **Join Request (`src/handlers/join_request.rs`)**:
   - Check `config.is_chat_allowed(req.chat.id)` (empty allowlist permits all chats).
   - Screen applicant bio (`src/screening.rs`): case-insensitive substring check (no regex crate) auto-declines immediately on `t.me/`, `telegram.me/`, `tg://`, `http://`, or `https://`.
   - Generate `SessionToken` (16 hex chars; validated `1..=32` ASCII alphanumeric) and `Challenge` (`1..=9 + 1..=9` with 3 distinct choices).
   - Persist via `store.upsert` **before** calling `sendMessage` to `req.user_chat_id`. On `Ok(msg)`, call `store.set_message_id`. On `Err`, call `store.remove_by_token` and leave request pending for `UserBlockedBot`/`DmWindowExpired`.
   - When `BOTIFIL_WEBAPP_URL` is set, DM sends `InlineKeyboardButton::web_app` with `?token=<token>` appended (preserving existing subpath/query); otherwise sends 3 inline callback buttons (`v1:<token>:<answer>`).
3. **Resolution (`src/handlers/callback.rs`, `src/webapp/mod.rs`)**:
   - Both inline callbacks and `POST /api/verify` call `store.verify_attempt` then shared `webapp::resolve_join_request`.
   - `chat_member` updates (`src/handlers/mod.rs`) evict stale sessions via `store.remove_by_peer(chat_id, user_id)`.

## 3. Telegram Domain Invariants
- **Three distinct IDs (never swap)**:
  - `req.chat.id` (`ChatId`) -> target group/channel for `approveChatJoinRequest` / `declineChatJoinRequest`.
  - `req.from.id` (`UserId`) -> applicant for `approveChatJoinRequest` / `declineChatJoinRequest`.
  - `req.user_chat_id` (`ChatId`) -> ephemeral DM target for `sendMessage` / `editMessageText` only.
- **5-Min DM Window & Resolve-Before-Feedback**: A join request grants a 300s DM ticket (no `/start`). Telegram emits no update when an admin dismisses a request, which also closes the DM ticket. Always call `approveChatJoinRequest` / `declineChatJoinRequest` **before** `answerCallbackQuery` and best-effort `editMessageText` so admin-dismissed requests never show false approvals.
- **Permissions & Invite Links**: Bot needs `can_invite_users`. `creates_join_request = true` is mutually exclusive with `member_limit` (`400 MEMBER_LIMIT_INVALID`).
- **Callback Limit**: `InlineKeyboardButton.callback_data` <= 64 UTF-8 bytes (`MAX_CALLBACK_DATA_BYTES`).
- **Error Classification (`src/error.rs`)**:
  - `400 HIDE_REQUESTER_MISSING` -> `IdempotentNoOp` (evict session, notify no longer pending).
  - `400 USER_ALREADY_PARTICIPANT` -> `IdempotentSuccess` (evict session, report already member).
  - `400 CHAT_ADMIN_REQUIRED` / `RIGHTS_INSUFFICIENT` -> `FatalAdminRequired`.
  - `400 USER_ID_INVALID` -> `BugInvalidUserId` (swapped ID bug).
  - `403 BotBlocked` / `USER_IS_BLOCKED` / `CantInitiateConversation` -> leave join request pending; do **not** auto-decline.
  - `429 RetryAfter` -> `RateLimited` (handled by `Bot::throttle`).

## 4. WebApp HTTP Server & Auth Contract (`src/webapp/`)
- **Transport (`server.rs`)**: Raw Tokio HTTP/1.1 server (zero web-framework crates), strips base path from `BOTIFIL_WEBAPP_URL`.
- **Routes**:
  - `GET /` -> embedded `ui.html` (`include_str!`, Rose-Pine theme).
  - `GET /healthz` -> `200 {"status":"ok"}`.
  - `GET /api/challenge?token=<tok>` -> non-mutating `get_by_token` returning `{left, right, choices, attempts_left, remaining_secs}` or `404 {"status":"expired"}`.
  - `POST /api/verify` -> `application/x-www-form-urlencoded` (`token`, `answer`, `init_data`), returns `200 {"status":"approved"|"declined"|"retry"}`, `403`, `404`, or `502`.
- **HTTP Hardening**: `MAX_HEADER_BYTES = 8192`, `MAX_BODY_BYTES = 16384`, `READ_TIMEOUT = 5s`; rejects `Transfer-Encoding` and duplicate/non-digit `Content-Length`; always sets `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Connection: close`; never leaks `chat_id` or bot token in responses.
- **Auth (`auth.rs`)**: Pure-Rust SHA-256 + HMAC-SHA256 (`HMAC_SHA256("WebAppData", bot_token)`), verifies sorted `k=v\n` check string, enforces `auth_date` within `challenge_ttl`, extracts `user.id`, and binds it to `PendingJoin.user_id` (`WrongUser` -> `403`).

## 5. Persistence & Binary Schema (`src/store/`)
- **`Storage` trait (`mod.rs`)**: `is_blocking()` controls whether `SessionStore` offloads calls via `tokio::task::spawn_blocking` (`RedbSessionStore` = `true`, `MemoryStore` = `false`). Never run `redb` I/O directly on async executor threads.
- **On-disk schema (`redb.rs`)**: Table `"pending_joins"` maps `&str` (token) -> fixed 44-byte little-endian `[u8; 44]`:
  `chat_id[0..8]`, `user_id[8..16]`, `user_chat_id[16..24]`, `left/right/c0/c1/c2/attempts/max_attempts[24..31]`, `msg_tag[31]` (`0`=None, `1`=Some), `msg_id[32..36]`, `expires_at_ms[36..44]`. Changing byte layout breaks existing `.redb` files.
- **Time anchoring & bounds**: `Instant` <-> Unix epoch ms uses startup `(anchor, anchor_ms)`. Capacity is bounded (`DEFAULT_MAX_CAPACITY`); `StoreError::CapacityExceeded` drops new requests gracefully.

## 6. Configuration, Security & Deployment
- **Config (`src/config.rs`, `.env.example`)**:
  - `TELOXIDE_TOKEN` (required, non-empty); `BOTIFIL_CHALLENGE_TTL_SECS` (`1..=300`, default `300`); `BOTIFIL_MAX_ATTEMPTS` (`1..=5`, default `2`); `BOTIFIL_ALLOWED_CHATS` (comma-separated `i64`, empty = all); `BOTIFIL_DB_PATH` (default `./data/botifil.redb`, empty string errors); `BOTIFIL_WEBAPP_URL` (`https://` or local `http://`, no userinfo, empty string errors, unset = inline buttons); `BOTIFIL_WEBAPP_BIND` (default `127.0.0.1:8080`); `CLOUDFLARE_TUNNEL_TOKEN` (compose sidecar); `RUST_LOG` (default `info`).
- **Redaction & Privacy**: `Config::Debug` redacts `telegram_token` and `db_path` (`[REDACTED]`) and logs only `allowed_chats_count`. Never log or commit tokens, group/user IDs, or personal filesystem paths; `gitleaks` scans working tree and full git history.
- **Container (`Containerfile`, `compose.yaml`)**: Multi-stage static `x86_64-unknown-linux-musl` build (`musl-tools` required for `ring` C compilation) on `distroless/static-debian12:nonroot` (`65532:65532`), `read_only: true`, `cap_drop: [ALL]`, `no-new-privileges:true`, `./data:/data` bind mount, paired with `cloudflared` sidecar.

## 7. Rust Standards, Testing & Dependencies
- **Compiler/Clippy gates (`Cargo.toml`)**: `unsafe_code = "forbid"`, `clippy::pedantic = "warn"`, `unwrap_used`/`expect_used`/`panic = "deny"` outside tests.
- **Conventions**: `thiserror` in library modules, `anyhow` only in `src/main.rs`, `tracing` for structured logs (never `println!`/`eprintln!`), immutable/pure helpers where practical.
- **Testing**: Unit tests inline or in `*_tests.rs`; integration tests in `tests/` (`gatekeeper_e2e.rs`, `persistence_test.rs`, `webapp_e2e.rs`) mock Telegram via `wiremock` (`Bot::new("test_token").set_api_url(...)`) and use `MemoryStore` or temp `redb` files—never hit live Telegram API.
- **Dependencies**: Ask maintainer before adding crates or ignoring `cargo-deny` advisories in `deny.toml`.

## 8. Git, PR & Documentation Rules
- **Branching**: Never commit directly to `main`; work in `.worktrees/<slug>` on `<type>/<slug>` off `origin/main`.
- **Commits & PRs**: One logical scope per PR; atomic, diff-derived Conventional Commits (never mix `refactor` with `feat`/`fix`; keep tests with their slice). Plain-word subjects/PR titles (no milestone IDs, phase numbers, or private tool names). Pass multi-line PR bodies/comments via `--body-file` with `./scripts/verify.sh` tail (`verify: OK`). Agents never approve, merge, or force-push without explicit maintainer instruction.
- **Docs Single Source of Truth**: Only three root docs exist—[README.md](README.md) (users/ops), [AGENTS.md](AGENTS.md) (agents/devs), [CHANGELOG.md](CHANGELOG.md) (history & architectural decisions). Update the relevant file when behavior or architecture changes.

## 9. Architecture Map
- `src/main.rs`: bootstrap, `getMe`, `redb` open + reconciliation, `Bot::throttle`, `Dispatcher`, reaper, WebApp server, shutdown.
- `src/config.rs`, `src/config_tests.rs`: validated env config and redacted `Debug`.
- `src/domain.rs`, `src/error.rs`, `src/screening.rs`: `SessionToken`, `Challenge`, `CallbackAction`, `PendingJoin`, `SessionOutcome`, `BotErrorAction`, bio substring screener.
- `src/store/`: `Storage` trait (`mod.rs`), `MemoryStore` (`memory.rs`, `memory_tests.rs`), `RedbSessionStore` (`redb.rs`), startup reconciliation (`reconciliation.rs`).
- `src/handlers/`: `dptree` schema & `chat_member` eviction (`mod.rs`), `join_request.rs`, `callback.rs`.
- `src/webapp/`: shared `resolve_join_request` & `INDEX_HTML` (`mod.rs`), HMAC-SHA256 `initData` verifier (`auth.rs`), HTTP/1.1 server (`server.rs`), embedded Mini App (`ui.html`).
- `tests/`: `gatekeeper_e2e.rs`, `persistence_test.rs`, `webapp_e2e.rs`.
- `Containerfile`, `compose.yaml`, `scripts/`: musl distroless build + `cloudflared` compose, `verify.sh`, `dev-tunnel.sh`, `gh-bootstrap.sh`.
