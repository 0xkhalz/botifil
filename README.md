# botifil

Telegram bot that approves or declines join requests for groups (Rust + teloxide).

**Status:** bootstrapping — architecture and agent contracts in progress.

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
