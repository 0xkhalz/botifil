# Agent-First PR Workflow

- Status: Accepted
- Date: 2026-11-13

## Context and Problem Statement

`botifil` is developed by a solo human maintainer collaborating with autonomous AI agents. Standard GitHub branch protection requiring peer review approvals blocks solo maintainers because GitHub forbids self-approving pull requests.

## Decision Drivers

- Enforce rigorous automated verification on all changes before landing on `main`.
- Accommodate a solo maintainer without requiring dummy approval accounts.
- Enable autonomous agent implementation in isolated git worktrees.
- Ensure all automated review feedback is resolved prior to merge.

## Considered Options

- GitHub Ruleset (`.github/rulesets/main.json`) with automated status checks
- GitHub Classic branch protection with 1 required approval
- Direct pushes to `main` without PR gates

## Decision Outcome

Chosen option: GitHub Ruleset (`.github/rulesets/main.json`) with 0 required approvals. Merges to `main` require green `ci-ok`, conventional `pr-title`, resolution of all review threads (`required_review_thread_resolution`), and squash-only merges. Agents develop in worktrees, reviewer agents post comments, and the maintainer merges.

## Consequences

- Positive: Unblocks solo-maintainer PR merges without compromising safety gates.
- Positive: Guarantees linear git history and complete CI validation.
- Negative: Requires discipline in tracking review thread comments before merge.
