# botifil

[![CI](https://github.com/0xkhalz/botifil/actions/workflows/ci.yml/badge.svg)](https://github.com/0xkhalz/botifil/actions/workflows/ci.yml)

Telegram bot that approves or declines join requests for groups (Rust + teloxide).

**Status:** M2 Gatekeeper implemented.

## Configuration

Configure runtime settings via environment variables or a `.env` file:

- `TELOXIDE_TOKEN` — Telegram bot token from @BotFather (required).
- `BOTIFIL_CHALLENGE_TTL_SECS` — Challenge timeout in seconds (1..=300, default: 300).
- `BOTIFIL_MAX_ATTEMPTS` — Max verification attempts before decline (1..=5, default: 2).
- `BOTIFIL_ALLOWED_CHATS` — Comma-separated list of allowed chat IDs (empty allows all).
- `RUST_LOG` — Tracing log filter (e.g. `info`, default: `info`).

## Verification

Run local verification before opening or updating pull requests:

```bash
./scripts/verify.sh
```

## How Development Works

1. Feature specification drafted and approved (`docs/specs/`).
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
