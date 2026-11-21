# botifil — Agent Instructions

Telegram bot that verifies and approves/declines group join requests (Rust + `teloxide`).

## 1. Toolchain & Runtime
- Rust `1.99` stable, edition `2024` (`rust-toolchain.toml`, `Cargo.toml`); tools via `mise install` (`mise.toml`, `CARGO_BUILD_JOBS=2`).
- Env config (`.env.example`): `TELOXIDE_TOKEN`, `BOTIFIL_CHALLENGE_TTL_SECS`, `BOTIFIL_MAX_ATTEMPTS`, `BOTIFIL_ALLOWED_CHATS`, `BOTIFIL_DB_PATH`, `BOTIFIL_WEBAPP_URL`, `BOTIFIL_WEBAPP_BIND`, `CLOUDFLARE_TUNNEL_TOKEN`, `RUST_LOG`.

## 2. Verification Pipeline
```bash
./scripts/verify.sh        # Full gate (must exit 0 before opening/updating PR)
./scripts/verify.sh --docs # typos, lychee, actionlint, gitleaks
./scripts/verify.sh --rust # fmt, clippy, nextest, doc tests, cargo-deny, cargo-shear
```

## 3. Non-Negotiable Invariants
- **Git & PRs**: Never commit to `main`; work in `.worktrees/<slug>` on `<type>/<slug>`. One logical scope per PR; atomic, diff-derived Conventional Commits (never mix `refactor` with `feat`/`fix`). Agents never approve, merge, or force-push (`gh pr merge --rebase` is maintainer-only).
- **Rust**: `unsafe` forbidden; no `unwrap`, `expect`, or `panic!` outside tests (`Cargo.toml` denies them). Use `thiserror` in library modules, `anyhow` only in `src/main.rs`, `tracing` for logs, `wiremock` for API tests, `spawn_blocking` for `redb` I/O, and `Bot::throttle` for rate limits.
- **Dependencies & Security**: Ask maintainer before adding crates. Never commit or log tokens, group/user IDs, or personal paths; redact secrets in `Debug`. Validate untrusted Telegram/HTTP input at boundaries; `gitleaks` must pass.
- **Docs Single Source of Truth**: Only three root docs exist: [README.md](README.md) (users/ops), [AGENTS.md](AGENTS.md) (dev/agent rules), [CHANGELOG.md](CHANGELOG.md) (changes & key decisions). Update the relevant one when behavior or architecture changes.

## 4. Telegram Domain Invariants
- **IDs (never swap)**: `req.chat.id` (`ChatId`) -> approve/decline target group; `req.from.id` (`UserId`) -> approve/decline user; `req.user_chat_id` (`ChatId`) -> DM `sendMessage`/`editMessageText` only.
- **5-Min DM Window & Ordering**: Join request grants a 300s DM ticket (no `/start`). Always resolve (`approveChatJoinRequest` / `declineChatJoinRequest`) **before** `answerCallbackQuery` and best-effort `editMessageText` so admin-dismissed requests never show false approvals.
- **Updates & Rights**: `allowed_updates` must include `chat_join_request`, `callback_query`, `chat_member`. Bot needs `can_invite_users`. `creates_join_request = true` is mutually exclusive with `member_limit`.
- **Callback & Errors**: Inline `callback_data` <= 64 UTF-8 bytes. Treat `400 HIDE_REQUESTER_MISSING` (idempotent no-op) and `400 USER_ALREADY_PARTICIPANT` (idempotent success) cleanly; leave pending on `403` or TTL expiry.

## 5. Architecture Map
- `src/main.rs`: startup (`getMe`, `redb` open + expiry reconciliation), `Bot::throttle`, `Dispatcher`, 5s reaper, optional WebApp HTTP server, graceful shutdown.
- `src/config.rs`, `src/domain.rs`, `src/error.rs`, `src/screening.rs`: validated env config, challenge/token models, Telegram error classifier, applicant bio regex screening.
- `src/store/`: `Storage` trait (`mod.rs`), bounded `MemoryStore` (`memory.rs`), `RedbSessionStore` (`redb.rs`), startup expiry reconciliation (`reconciliation.rs`).
- `src/handlers/`: `dptree` schema & `chat_member` eviction (`mod.rs`), `join_request.rs`, `callback.rs`.
- `src/webapp/`: shared `resolve_join_request` (`mod.rs`), HMAC-SHA256 `initData` verifier (`auth.rs`), Tokio HTTP/1.1 server (`/`, `/api/verify`, `/healthz` in `server.rs`), embedded Mini App (`ui.html`).
- `tests/`: `gatekeeper_e2e.rs`, `persistence_test.rs`, `webapp_e2e.rs` (`wiremock` suites).
- `Containerfile`, `compose.yaml`, `scripts/`: static musl distroless container + `cloudflared` sidecar, `verify.sh`, `dev-tunnel.sh`, `gh-bootstrap.sh`.
