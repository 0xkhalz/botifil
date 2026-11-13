# botifil — Agent Instructions

Telegram bot that approves/declines group join requests. Rust + teloxide. Agents implement; maintainer reviews and merges.

## 1. Toolchain & Runtime
- Rust: 1.99 stable, edition 2024 (`rust-toolchain.toml`).
- Tooling: managed via `mise install` (`mise.toml`, `CARGO_BUILD_JOBS=2` for constrained local dev).
- Secrets: set via environment or `.env` (`TELOXIDE_TOKEN`); never commit `.env` or secrets.

## 2. Verification Pipeline
```bash
./scripts/verify.sh # must exit 0 before opening/updating PR; paste tail in PR body
```

## 3. Non-Negotiable Invariants
- Never commit directly to `main`.
- 1 issue → 1 `.worktrees/` branch `<type>/<issue#>-<slug>` → 1 PR.
- Agents never merge, approve, force-push, or edit repository rulesets/secrets.
- PR size ≤ 300 LoC diff (excl. `Cargo.lock`); files ≤ 300 lines.
- Every commit and PR title must follow Conventional Commits (rebase-merge preserves all commits).
- No `unwrap`, `expect`, or `panic!` outside tests; `unsafe` forbidden.
- No new dependencies without an accepted ADR.
- Never commit or log tokens, chat/group IDs, or personal filesystem paths.
- Features require an approved spec in `docs/specs/`; architectural changes require an ADR in `docs/decisions/`.
- Ambiguous scope or 3 failed verify runs → apply label `status:needs-human` and stop.

## 4. Architecture Map
- `src/`: future bot implementation (teloxide dispatcher, filters, handlers).
- `docs/domain/`: future Telegram API domain models and join-request mechanics.
- `docs/decisions/`: future Architecture Decision Records (ADRs).
- `docs/specs/`: future implementation specifications and PRDs.
- `.agents/rules/`: agent behavior and engineering rules.
- `.agents/skills/`: future task-specific agent workflows.
- `.github/`: CI workflows and automated checks.
- `scripts/`: local verification and repository tooling.
