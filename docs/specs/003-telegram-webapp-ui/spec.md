# Specification: Telegram Web App Verification UI

- Status: draft
- Date: 2026-11-15
- Milestone: M5

## Motivation & Context

Inline keyboard math challenges in M2 provide minimal feedback and lack a live countdown or dedicated verification surface. Milestone M5 adds an embedded Telegram Mini App (Web App) verification UI served directly by the bot binary, keeping the low-RAM pure-Rust baseline ([ADR 0002](../../decisions/0002-rust-teloxide-framework.md), [ADR 0005](../../decisions/0005-rust-quality-and-low-ram-baseline.md)), read-only static musl distroless compatibility ([ADR 0004](../../decisions/0004-static-musl-distroless-deploy.md)), `redb` persistence ([ADR 0006](../../decisions/0006-persistence-engine.md)), and crate selections proposed in [ADR 0007](../../decisions/0007-telegram-webapp-verification-ui.md).

## User-Facing Behavior

1. **WebApp DM Button & Inline Fallback**: When `BOTIFIL_WEBAPP_URL` is configured, the verification DM sends a single `InlineKeyboardButton::web_app("🛡️ Open Verification", ...)` pointing to the session URL (`?token=<token>`). When `BOTIFIL_WEBAPP_URL` is unset, the bot falls back to the M2 3-button inline math challenge.
2. **Embedded Rose-Pine Mini App**: Tapping the button opens Telegram's embedded Mini App styled in the Rose-Pine palette, rendering `What is A + B?`, 3 randomized choice buttons, a live TTL countdown bar, and an attempt counter. `ui.html` calls relative `./api/challenge` and `./api/verify` endpoints so subpath proxy deployments work transparently.
3. **Submission, Error Messaging & Auto-Close**: Selecting an option submits `{ token, answer, init_data }` with `Telegram.WebApp.initData`. On wrong answer with remaining attempts, the UI updates the attempt counter; on `403`, it distinguishes `wrong_user` (opened by a different Telegram account) from `invalid_auth` (invalid or expired `initData`); on terminal `approved`, `declined`, or `expired`, it displays the final state and calls `Telegram.WebApp.close()`.

## Technical Requirements

### Configuration
- `BOTIFIL_WEBAPP_URL`: Optional base URL. Must use `https://`, except `http://` is allowed strictly for `localhost`, `127.0.0.1`, or `[::1]`. Invalid or empty non-unset values fail startup (`ConfigError`).
- `BOTIFIL_WEBAPP_BIND`: Optional `SocketAddr` bind address, defaulting to `127.0.0.1:8080`.
- Preserve M3 `BOTIFIL_DB_PATH` configuration and redaction invariants.

### Non-Mutating Store Lookup
- Extend `Storage` and `SessionStore` (`MemoryStore`, `RedbSessionStore`) with `get_by_token(&self, tok: &SessionToken, now: Instant) -> Option<PendingJoin>` returning active, unexpired sessions without mutating attempts or removing records.
- `RedbSessionStore::get_by_token` MUST use a read-only `redb` transaction (`begin_read()`) for unexpired lookups and open a write transaction only when lazy-evicting an expired entry.

### Pure-Rust `WebAppData` Verification
- Compute `secret = HMAC-SHA256("WebAppData", bot_token)` and verify `hash = HMAC-SHA256(secret, data_check_string)` in constant time over sorted `key=value` `\n`-joined `initData` fields.
- Validate freshness `now.saturating_sub(auth_date) <= challenge_ttl_secs` with up to `60s` future clock skew (`auth_date <= now + 60`), and extract authenticated `UserId` for `verify_attempt` ownership checks.

### HTTP Endpoints & Ordering Invariant
| Endpoint | Behavior & Status Codes |
| --- | --- |
| `GET /` | Serves compile-time embedded Rose-Pine `ui.html` (`text/html; charset=utf-8`), stripping any configured `BOTIFIL_WEBAPP_URL` base path. |
| `GET /api/challenge?token=...` | Validates `SessionToken`, calls `get_by_token`, returns prompt, choices, attempt counts, and remaining TTL (`200`, `400`, `404`). |
| `POST /api/verify` | Verifies `initData` + `UserId`, calls `verify_attempt`, returns `{ status }` (`retry`, `approved`, `declined`, `expired`) or `403` (`wrong_user` vs `invalid_auth`), `404` on missing/expired token, `400` on malformed/chunked/duplicate `Content-Length`, `408` on read timeout. |

- **DM-Edit-Before-Resolve Invariant**: Before calling `approve_chat_join_request` or `decline_chat_join_request`, the server MUST call `edit_message_text` on `user_chat_id` + `message_id` (matching `callback.rs` and `classify_request_error` idempotency).
- **M3 Preservation**: Preserve `SessionStore::open_redb`, `reconcile_on_startup`, and `StoreError::Backend` error propagation in `src/main.rs` and `src/handlers/join_request.rs`.

## Out of Scope

- Built-in TLS termination (handled by reverse proxy in front of `BOTIFIL_WEBAPP_BIND`).
- Admin web dashboard, analytics views, or multi-node session synchronization.
- External CAPTCHA providers (Turnstile, hCaptcha, image challenges).

## Verification & Acceptance

- Unit tests cover `BOTIFIL_WEBAPP_URL` (`https://`, `localhost`/`127.0.0.1`/`[::1]` `http://`) and `BOTIFIL_WEBAPP_BIND`, `Storage::get_by_token` (including `RedbSessionStore` read-only path), and pure-Rust HMAC-SHA256 `initData` signature + freshness/skew checks.
- Integration tests in `tests/webapp_e2e.rs` verify `GET /`, subpath routing, `GET /api/challenge`, `POST /api/verify`, expired `POST` `404`, expired `initData` `403`, `wrong_user` vs `invalid_auth` `403`, `chunked`/duplicate `Content-Length` `400`, `408`, inline fallback, and `editMessageText` ordering via `wiremock`.
- `./scripts/verify.sh` exits `0`.
