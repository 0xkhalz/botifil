---
description: "Git branching, worktree isolation, commit hygiene, and pull request conventions"
trigger: always_on
---

# Git Workflow

## Branching & Worktrees
- Branch naming: `<type>/<issue#>-<slug>` (e.g. `feat/12-add-captcha`).
- Always create branches in a flat isolated worktree (max 2–3 concurrent worktrees/subagents for RAM ceiling):
  ```bash
  git worktree add .worktrees/<issue#>-<slug> -b <type>/<issue#>-<slug> origin/main
  ```
- After PR rebase-merge, remove the worktree and branch:
  ```bash
  git worktree remove .worktrees/<issue#>-<slug> && git branch -d <type>/<issue#>-<slug>
  ```

## Staging & Commits
- **One job = one commit**: commit continuously at each logical change boundary (no end-of-task dump commits). Never bundle unrelated jobs regardless of line count.
- **Never mix change types**: keep `refactor` separate from `feat`/`fix` (commit preparatory refactors first with tests passing); keep tests with their slice (standalone test work as `test:`).
- **Change-based messages**: name commits after the exact change made using Conventional Commits (never a task title, phase name, PR title, or subagent assignment).
- Stage explicit file paths (`git add <path1> <path2>`) or hunks (`git add -p`); never use `git add -A` or `git add .`.
- Pre-commit gate: inspect `git diff --cached` before every commit; if staged changes span multiple jobs, unstage and commit each job separately.
- **Three-tier ignore & security gate**: never add personal AI directories to `.gitignore` and never stage transient/non-essential files (`GEMINI.md`, `*_PLAN.md`, screenshots, ad-hoc scripts). Verify no secrets, validated boundary inputs, and no sensitive data in errors before committing.
- Pre-PR gate: inspect `git log --stat origin/main..HEAD` before opening or updating a PR to verify one clean commit per job.

## Pull Requests
- **One logical scope per PR**: never bundle unrelated features, fixes, or refactors into one PR; PR title must follow Conventional Commits.
- Pass multi-line PR bodies and comments via `--body-file` or `-F` (never inline strings).
- Reference issues using `Fixes #N` or `Closes #N` (repeat keyword for multiple: `Fixes #1, Fixes #2`).

## Synchronization & Prohibitions
- Rebase on `origin/main` before opening or updating PRs; resolve only session-modified files, and abort and ask the user on foreign conflicts.
- Run `git status` before any command that could overwrite or discard uncommitted changes.
- Never commit directly to `main`.
- Strictly prohibited without explicit user request: `git reset --hard`, `git checkout .`, `git restore .`, `git clean -fd`, `git stash`, `git branch -D`, `git push --force` / `--force-with-lease`, and `--no-verify` / `-n`.
