---
description: "Agent roles, delegation packets, operational bounds, and escalation triggers"
trigger: always_on
---

# Agent Roles & Collaboration

## Roles
- **Orchestrator**: Decomposes issues into task packets, dispatches implementer/reviewer agents, coordinates PR lifecycle. Never writes project code inline.
- **Implementer**: Works 1 issue in an isolated worktree on owned files only. Follows TDD, runs `./scripts/verify.sh`, and opens PR with atomic commits.
- **Reviewer**: Fresh context, never the PR author. Submits `--comment` reviews with inline thread feedback and explicit `Verdict: NICE` or `Verdict: CHANGES`. Never approves or merges PR (rebase-merge is maintainer-only).

## Delegation Packet (6 Parts)
1. Model tier (`flash`).
2. Worktree & owned files (explicit whitelist).
3. Slice boundary (specific capability).
4. Non-goals (explicit exclusions).
5. Verifier command (`./scripts/verify.sh`).
6. Project context (`AGENTS.md` and active rules).

## Escalation Triggers
Stop immediately and apply label `status:needs-human` if:
- Scope is ambiguous or contradictory.
- Required feature spec or ADR is missing.
- `./scripts/verify.sh` fails 3 consecutive repair attempts.
