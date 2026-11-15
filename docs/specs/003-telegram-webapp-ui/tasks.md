# Tasks: Telegram Web App Verification UI

- Spec: [spec.md](./spec.md)
- Plan: [plan.md](./plan.md)

## Slices

### Task 1: M5-1 WebApp Config & Non-Mutating Store Lookup
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**:
  - `src/config.rs`
  - `src/store/memory.rs`
  - `src/store/mod.rs`
  - `src/store/redb.rs`
- **Slice Boundary**: Parse/validate `BOTIFIL_WEBAPP_URL` (`https://`, or `http://` for `localhost`/`127.0.0.1`) and `BOTIFIL_WEBAPP_BIND` (`127.0.0.1:8080` default); add non-mutating `Storage::get_by_token` and `SessionStore::get_by_token`.
- **Non-Goals**: HTTP server, HMAC verification, HTML UI, handler changes.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 2: M5-2 Pure-Rust WebApp `initData` HMAC Auth
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**:
  - `Cargo.toml`
  - `src/lib.rs`
  - `src/webapp/auth.rs`
  - `src/webapp/mod.rs`
- **Slice Boundary**: Pure-Rust `WebAppData` HMAC-SHA256 signature verification, `now - auth_date <= challenge_ttl_secs` check, and `UserId` extraction per ADR 0007.
- **Non-Goals**: HTTP routes, HTML UI, join/callback handler updates.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 3: M5-3 Embedded Rose-Pine Mini App UI
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**:
  - `src/webapp/mod.rs`
  - `src/webapp/ui.html`
- **Slice Boundary**: Self-contained Rose-Pine HTML/CSS/JS Mini App (`What is A + B?`, 3 choices, TTL countdown bar, attempt counter, `Telegram.WebApp.initData` submit, `Telegram.WebApp.close()`).
- **Non-Goals**: HTTP server routing, `main.rs` wiring, `handlers/`.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 4: M5-4 WebApp HTTP Server & DM Resolution Ordering
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**:
  - `src/handlers/callback.rs`
  - `src/webapp/mod.rs`
  - `src/webapp/server.rs`
- **Slice Boundary**: Serve `GET /`, `GET /api/challenge`, and `POST /api/verify`; extract shared `editMessageText`-before-approve/decline helper reused by `callback.rs`.
- **Non-Goals**: `join_request.rs` WebApp button, `main.rs` startup wiring.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 5: M5-5 Join Request Button, Shutdown Wiring & E2E Tests
- **Parent Spec/Issue**: docs/specs/003-telegram-webapp-ui/spec.md
- **Owned Files**:
  - `.env.example`
  - `src/handlers/join_request.rs`
  - `src/main.rs`
  - `tests/gatekeeper_e2e.rs`
- **Slice Boundary**: Send `InlineKeyboardButton::web_app` when `BOTIFIL_WEBAPP_URL` is set (inline fallback when unset), spawn/drain WebApp server in `main.rs`, and add `wiremock` E2E tests.
- **Non-Goals**: Containerfile changes, TLS termination, admin dashboard.
- **Verifier Command**: `./scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly
