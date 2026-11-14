# botifil

[![CI](https://github.com/0xkhalz/botifil/actions/workflows/ci.yml/badge.svg)](https://github.com/0xkhalz/botifil/actions/workflows/ci.yml)

Telegram bot that approves or declines join requests for groups (Rust + teloxide).

## Status

- **M1 Foundation & M2 Join-Request Gatekeeper**: Implemented and verified.
- **M3 Persistence & Admin Controls**: Specified in [002 spec](docs/specs/002-persistence-and-admin-controls/spec.md) and [ADR 0006](docs/decisions/0006-persistence-engine.md).

## Gatekeeper Behavior

- **Group Allowlist**: Filters requests by `BOTIFIL_ALLOWED_CHATS` (empty allows all configured groups).
- **Bio Screening**: Detects spam URLs and invite links in applicant user bios (Bot API 10.1+).
- **Arithmetic DM Challenge**: Sends a math captcha DM to the applicant `user_chat_id`.
- **Edit-Before-Resolve**: Updates the DM challenge message before resolving join requests to avoid race conditions.
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
| `RUST_LOG` | Tracing log filter level | `trace`, `debug`, `info`, `warn`, `error` | `info` |

## Repository Layout

| Directory | Purpose |
| --- | --- |
| `src/` | Bot dispatcher, config, domain models, handlers, screening, and store |
| `tests/` | End-to-end integration tests using `wiremock` |
| `docs/` | Roadmap, Telegram mechanics, MADR 4 ADRs, and implementation specs |
| `.agents/` | Agent rules, roles, workflows, and task skills |
| `scripts/` | Local verification and developer tooling |

## Verification

Run local verification before opening or updating pull requests:

```bash
./scripts/verify.sh         # Run full verification (docs + rust)
./scripts/verify.sh --docs  # Fast docs check (typos, lychee, actionlint, gitleaks, line limits)
./scripts/verify.sh --rust  # Rust toolchain checks (fmt, clippy, nextest, doc tests, deny, shear)
```

## Development Workflow

1. Feature specification drafted and approved in [docs/specs/README.md](docs/specs/README.md).
2. GitHub issue created with `status:ready`.
3. Agent implementer branches in isolated worktree (`.worktrees/`).
4. Local verification (`./scripts/verify.sh`) passes before PR creation.
5. GitHub Actions verify `ci-ok` and `pr-title`.
6. Reviewer agent reviews and comments; implementer resolves feedback.
7. Maintainer rebase-merges PR to `main` (`gh pr merge --rebase`).

## Documentation

- [AGENTS.md](AGENTS.md) — Agent instructions, constraints, and architecture map.
- [CONTRIBUTING.md](CONTRIBUTING.md) — Contribution workflow and collaboration protocol.
- [SECURITY.md](SECURITY.md) — Security policy and vulnerability reporting.
- [docs/roadmap.md](docs/roadmap.md) — Project milestones and execution roadmap.
- [docs/domain/telegram-join-requests.md](docs/domain/telegram-join-requests.md) — Telegram join request mechanics.
- [docs/specs/README.md](docs/specs/README.md) — Specification index and templates.
- [docs/decisions/README.md](docs/decisions/README.md) — Architecture Decision Records index.
