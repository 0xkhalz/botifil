---
description: "Agent roles, delegation packets, operational bounds, and escalation triggers"
trigger: always_on
---

# Agent Roles & Collaboration

## Roles
- **Orchestrator**: Decomposes issues into task packets, dispatches implementer/reviewer agents with `Model="inherit"` (never `flash` or `flash_lite`), and coordinates the PR lifecycle. Never writes project code inline and never runs `git add` or `git commit` on project code (permitted git ops: `merge`, `rebase`, `worktree`, `branch`, `log`, `diff`, `status`). Provisions `.worktrees/<issue#>-<slug>` before dispatch (max 2–3 concurrent worktrees/subagents), checks `git status` is clean before the next dispatch (sending the subagent back if dirty), and verifies `git log --stat origin/main..HEAD` shows atomic commits before integration/review.
- **Implementer**: Works 1 issue in an isolated worktree on owned files only. Follows TDD, runs `./scripts/verify.sh`, and opens PR. Owns all commits and commits continuously at each logical change boundary (no end-of-task dump commits): never mixes change types (`refactor` vs `feat` vs `fix`; preparatory refactors committed first with tests passing), keeps tests with their slice (standalone test work as `test:`), and uses change-based Conventional Commit messages (exact change made, never task title, phase name, PR title, or subagent assignment).
- **Reviewer**: Dispatched with `Model="inherit"` in a fresh context, never the PR author. Reviews code and commit history (flagging `Verdict: CHANGES` if distinct jobs are bundled, change types are mixed, or commit messages are not change-based), submits `--comment` reviews with inline thread feedback and explicit `Verdict: NICE` or `Verdict: CHANGES`, and passes multi-line comments via `--body-file` or `-F` (never inline strings). Never approves or merges PR (rebase-merge is maintainer-only).

## Delegation Packet
1. **Model Tier**: `Model="inherit"` (never `flash` or `flash_lite`).
2. **Worktree & Asset/File Ownership**: explicit whitelist.
3. **Slice Boundary**: specific capability/function.
4. **Non-Goals**: explicit exclusions (anti-wandering).
5. **Verifier**: `./scripts/verify.sh` (and relevant targeted tests).
6. **Project Context & Commit Rule**: inspect `AGENTS.md` and active rules; own all commits; commit continuously at each logical change boundary; verify `git diff --cached` and `git log --stat origin/main..HEAD`.

## Escalation Triggers
Stop immediately and apply label `status:needs-human` if:
- Scope is ambiguous or contradictory.
- A linked spec or required ADR is missing.
- `./scripts/verify.sh` fails 3 consecutive repair attempts.
