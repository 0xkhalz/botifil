---
description: "Git branching, worktree isolation, commit hygiene, and pull request conventions"
trigger: always_on
---

# Git Workflow

## Branching & Worktrees
- Branch naming: `<type>/<issue#>-<slug>` (e.g. `feat/12-add-captcha`).
- Always create branches in an isolated worktree:
  ```bash
  git worktree add .worktrees/<branch> -b <branch> origin/main
  ```
- After PR rebase-merge, remove the worktree and branch:
  ```bash
  git worktree remove .worktrees/<branch> && git branch -d <branch>
  ```

## Staging & Commits
- **One job = one commit**: each commit must represent one single logical job or concern. Never bundle two different jobs into one commit regardless of line count (e.g. 1 edit in `main` + 1 edit in `login` = 2 commits).
- `≤ 300 LoC` is the **PR** diff ceiling (excl. `Cargo.lock`), not a target for a single monolithic commit.
- Stage explicit file paths (`git add <path1> <path2>`) or hunks (`git add -p`); never use `git add -A` or `git add .`.
- Pre-commit gate: inspect `git diff --cached` before every commit; if staged changes span multiple jobs, unstage and commit each job separately.
- Pre-PR gate: inspect `git log --stat origin/main..HEAD` before opening or updating a PR to verify one clean commit per job.
- Every commit must follow Conventional Commits (rebase-merge preserves history on `main`).
- Reference issues using `Fixes #N` or `Closes #N`.

## Synchronization & Prohibitions
- Rebase branch on `origin/main` before opening or updating PRs.
- Never commit directly to `main`.
- Never force-push (`git push --force` or `--force-with-lease` are prohibited for agents).
