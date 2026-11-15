# Tasks: Telegram Web App Verification UI

- Spec: [spec.md](./spec.md)
- Plan: [plan.md](./plan.md)

## Slices

### Task 1: M5-1 WebApp Config & Dependencies
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**: `Cargo.toml`, `.env.example`, `src/config/mod.rs`, `src/config/tests.rs` (split from `src/config.rs`)
- **Slice Boundary**: Promote `url = "2"` and enable `tokio` `net` + `io-util`; parse/validate `BOTIFIL_WEBAPP_URL` (`https://`, or `http://` for `localhost`, `127.0.0.1`, `[::1]`) and `BOTIFIL_WEBAPP_BIND` (`127.0.0.1:8080` default), preserving `BOTIFIL_DB_PATH`.
- **Non-Goals**: Store changes, HTTP server, HMAC verification, UI, handler wiring.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Every file <= 300 lines and diff <= 300 LoC
  - [ ] `./scripts/verify.sh` exits 0

### Task 2: M5-2 Non-Mutating Store Lookup
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**: `src/store/mod.rs`, `src/store/memory.rs` (split if needed for `<= 300` lines), `src/store/redb.rs`
- **Slice Boundary**: Add non-mutating `Storage::get_by_token` and `SessionStore::get_by_token`; use `begin_read()` in `RedbSessionStore::get_by_token` for unexpired reads and open a write transaction only for lazy expiry eviction.
- **Non-Goals**: Config changes, HTTP server, HMAC verification, handlers.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Every file <= 300 lines and diff <= 300 LoC
  - [ ] `./scripts/verify.sh` exits 0

### Task 3: M5-3 Pure-Rust WebApp `initData` Verifier
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**: `src/lib.rs`, `src/webapp/mod.rs`, `src/webapp/auth.rs`
- **Slice Boundary**: Implement pure-Rust `WebAppData` HMAC-SHA256 verification with constant-time digest comparison, freshness `now.saturating_sub(auth_date) <= challenge_ttl_secs` (`auth_date <= now + 60`), and `UserId` extraction per ADR 0007.
- **Non-Goals**: HTTP routes, `ui.html`, join/callback handlers, `main.rs`.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Every file <= 300 lines and diff <= 300 LoC
  - [ ] `./scripts/verify.sh` exits 0

### Task 4: M5-4 Embedded Rose-Pine Mini App UI
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**: `src/webapp/mod.rs`, `src/webapp/ui.html`
- **Slice Boundary**: Author self-contained Rose-Pine HTML/CSS/JS Mini App using relative `./api/challenge` and `./api/verify` endpoints, rendering challenge/TTL/attempts, distinct `wrong_user` vs `invalid_auth` `403` UI messages, and `Telegram.WebApp.close()`.
- **Non-Goals**: HTTP server routing, `handlers/`, `main.rs`.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Every file <= 300 lines and diff <= 300 LoC
  - [ ] `./scripts/verify.sh` exits 0

### Task 5: M5-5 Embedded HTTP/1.1 Server & Shared DM Resolution Helper
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**: `src/webapp/mod.rs`, `src/webapp/server.rs`, `src/handlers/callback.rs`
- **Slice Boundary**: Implement `GET /`, `GET /api/challenge`, and `POST /api/verify` with `BOTIFIL_WEBAPP_URL` subpath stripping, `chunked`/duplicate `Content-Length` `400`, `408` timeout, `EMFILE` accept backoff, and shared `editMessageText`-before-resolve helper reused by `callback.rs`.
- **Non-Goals**: `join_request.rs` WebApp button, `main.rs` wiring, E2E test suite.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Every file <= 300 lines and diff <= 300 LoC
  - [ ] `./scripts/verify.sh` exits 0

### Task 6: M5-6 Join-Request WebApp DM Button & Startup/Shutdown Wiring
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**: `src/handlers/join_request.rs`, `src/main.rs`
- **Slice Boundary**: Send `InlineKeyboardButton::web_app` when `BOTIFIL_WEBAPP_URL` is set (inline fallback when unset), preserving `StoreError::Backend` error propagation, `SessionStore::open_redb`, and `reconcile_on_startup`; spawn/drain WebApp server in `main.rs`.
- **Non-Goals**: E2E tests, Containerfile changes, TLS termination.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Every file <= 300 lines and diff <= 300 LoC
  - [ ] `./scripts/verify.sh` exits 0

### Task 7: M5-7 WebApp `wiremock` E2E Tests
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**: `tests/webapp_e2e.rs`
- **Slice Boundary**: Add `wiremock` E2E coverage for `GET /`, subpath routing, `GET /api/challenge`, `POST /api/verify`, expired `POST` `404`, expired `initData` `403`, `chunked` / duplicate `Content-Length` `400`, `408`, inline fallback, and `editMessageText` ordering.
- **Non-Goals**: Modifying production code under `src/`.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] File <= 300 lines and diff <= 300 LoC
  - [ ] `./scripts/verify.sh` exits 0
