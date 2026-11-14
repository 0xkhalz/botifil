# botifil — Agent Instructions

Telegram bot that approves/declines group join requests. Rust + teloxide. Agents implement; maintainer reviews and merges.

## 1. Toolchain & Runtime
- Rust: 1.99 stable, edition 2024 (`rust-toolchain.toml`, `Cargo.toml`).
- Tooling: managed via `mise install` (`mise.toml`, `CARGO_BUILD_JOBS=2` for constrained local dev).
- Secrets: set via environment or `.env` (`.env.example`: `TELOXIDE_TOKEN`, `BOTIFIL_CHALLENGE_TTL_SECS`, `BOTIFIL_MAX_ATTEMPTS`, `BOTIFIL_ALLOWED_CHATS`, `RUST_LOG`); never commit `.env` or secrets.

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
- Large multi-PR features use an approved spec in `docs/specs/`; single-PR tasks use the GitHub Issue directly. Architectural changes require an ADR in `docs/decisions/`.
- Ambiguous scope or 3 failed verify runs → apply label `status:needs-human` and stop.

## 4. Architecture Map
- `src/main.rs`: binary entry (`Bot::throttle`, `getMe` check, `Dispatcher`, 5s reaper, graceful shutdown).
- `src/lib.rs`, `src/config.rs`, `src/domain.rs`, `src/error.rs`, `src/screening.rs`, `src/store.rs`: config, challenge domain models, error classifier, bio screening, bounded `RwLock` store.
- `src/handlers/`: `dptree` schema (`mod.rs`), `join_request.rs`, `callback.rs`.
- `tests/gatekeeper_e2e.rs`: `wiremock` end-to-end join-request and callback suite.
- `docs/domain/`, `docs/decisions/`, `docs/specs/`: Telegram mechanics, MADR 4 ADRs (`0001`–`0006`), feature specs (`001`, `002`).
- `.agents/rules/`, `.agents/skills/`, `.github/`, `scripts/`: engineering rules, workflows, CI/rulesets, `verify.sh`.
