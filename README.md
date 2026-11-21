# botifil

[![CI](https://github.com/0xkhalz/botifil/actions/workflows/ci.yml/badge.svg)](https://github.com/0xkhalz/botifil/actions/workflows/ci.yml)

Telegram bot that approves or declines join requests for groups (Rust + teloxide).

## Gatekeeper Behavior

- **Group Allowlist**: Filters requests by `BOTIFIL_ALLOWED_CHATS` (empty allows all configured groups).
- **Bio Screening**: Detects spam URLs and invite links in applicant user bios (Bot API 10.1+).
- **Arithmetic DM Challenge**: Sends a math captcha DM to the applicant `user_chat_id`.
- **Resolve-Before-Feedback**: Calls `approveChatJoinRequest` / `declineChatJoinRequest` before answering callback queries and editing the DM, preventing false approvals on admin-dismissed requests.
- **Optional WebApp Verification**: Serves an embedded Telegram Mini App verified via HMAC-SHA256 `initData` when `BOTIFIL_WEBAPP_URL` is set (falls back to inline buttons otherwise).
- **Restart-Safe Persistence**: Persists pending sessions in an embedded `redb` database with startup expiry reconciliation.
- **Retry Attempts**: Supports configurable challenge retries via `BOTIFIL_MAX_ATTEMPTS`.
- **Idempotent Error Handling**: Treats already-resolved requests (`400 HIDE_REQUESTER_MISSING`) safely.
- **Leave-Pending Policy**: Leaves requests pending on `403 Forbidden` (blocked bot) or TTL expiry; auto-declines only on exhausted attempts or failed bio screening.

## Quickstart

```bash
mise install
cp .env.example .env
# edit .env and set TELOXIDE_TOKEN
cargo run --locked
```

## Configuration

| Variable | Description | Range / Format | Default |
| --- | --- | --- | --- |
| `TELOXIDE_TOKEN` | Telegram bot token from @BotFather | Non-empty string | *Required* |
| `BOTIFIL_CHALLENGE_TTL_SECS` | Challenge timeout in seconds | `1..=300` | `300` |
| `BOTIFIL_MAX_ATTEMPTS` | Maximum challenge answer attempts before decline | `1..=5` | `2` |
| `BOTIFIL_ALLOWED_CHATS` | Comma-separated list of allowed chat IDs | Integer IDs (empty allows all) | *(empty)* |
| `BOTIFIL_DB_PATH` | Persistent redb session database path | File path | `./data/botifil.redb` |
| `BOTIFIL_WEBAPP_URL` | HTTPS base URL for Telegram WebApp verification (unset = inline buttons) | `https://...` URL | *(unset)* |
| `BOTIFIL_WEBAPP_BIND` | Listen address for the embedded WebApp HTTP server | `host:port` | `127.0.0.1:8080` |
| `CLOUDFLARE_TUNNEL_TOKEN` | Cloudflare Tunnel token for the cloudflared Podman Compose sidecar | Token string | *(unset)* |
| `RUST_LOG` | Tracing log filter level | `trace`, `debug`, `info`, `warn`, `error` | `info` |

## Cloudflare Tunnel (WebApp)

### Local Quick Tunnel

Requires `cloudflared` installed on `PATH`. Run:

```bash
./scripts/dev-tunnel.sh
```

The script starts `cloudflared` against `BOTIFIL_WEBAPP_BIND` (default `127.0.0.1:8080`), waits for the ephemeral `https://*.trycloudflare.com` URL, and launches `cargo run --locked` with `BOTIFIL_WEBAPP_URL` set automatically.

### Production (Podman + Named Tunnel)

1. Set `BOTIFIL_WEBAPP_URL` (your HTTPS public hostname) and `CLOUDFLARE_TUNNEL_TOKEN` (used by the `cloudflared` Podman Compose sidecar) in `.env`.
2. In Cloudflare Zero Trust, point the tunnel's public hostname origin to `http://botifil:8080`.
3. Ensure `./data` exists and is writable by the container non-root user (`65532:65532`). For rootless Podman:

   ```bash
   mkdir -p ./data && podman unshare chown 65532:65532 ./data
   ```

   For rootful Podman or Docker, use `mkdir -p ./data && chown 65532:65532 ./data` instead.

4. Build and start the stack:

   ```bash
   podman compose up -d --build
   ```

### Cloudflare WAF / Bot Fight Mode

Telegram opens the Mini App inside an in-app WebView, which does not pass browser-based JS or Turnstile challenges. Bypass or disable Bot Fight Mode and JS/Turnstile challenges on the WebApp hostname/path in Cloudflare WAF, otherwise `GET /` and `POST ./api/verify` will fail inside Telegram.

## Repository Layout

| Directory | Purpose |
| --- | --- |
| `src/` | Bot dispatcher, config, domain models, handlers, screening, and store |
| `tests/` | End-to-end integration tests using `wiremock` |
| `scripts/` | Local verification and developer tooling |

## Verification

Run local verification before opening or updating pull requests:

```bash
./scripts/verify.sh         # Run full verification (docs + rust)
./scripts/verify.sh --docs  # Fast docs check (typos, lychee, actionlint, gitleaks)
./scripts/verify.sh --rust  # Rust toolchain checks (fmt, clippy, nextest, doc tests, deny, shear)
```

## Security

- **Vulnerability Reporting**: Report vulnerabilities privately via GitHub Security Advisories (**Security** > **Advisories** > **Report a vulnerability**); do not open public issues.
- **Secret Management**: Supply `TELOXIDE_TOKEN` only via environment variables or an uncommitted `.env`; never commit tokens or chat IDs.
- **Token Leak Response**: Revoke immediately via `@BotFather` (`/revoke`), generate a new token (`/token`), and rotate deployment secrets.

## Documentation

- [AGENTS.md](AGENTS.md) — Agent instructions, constraints, and architecture map.
- [CHANGELOG.md](CHANGELOG.md) — Unreleased changes and key architectural decisions.
