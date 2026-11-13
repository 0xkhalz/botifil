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
- Use explicit file paths (`git add <path1> <path2>`); never use `git add -A` or `git add .`.
- Atomic commits by change, not task (ceiling: ≤ 300 lines of code per commit).
- Every commit must follow Conventional Commits (rebase-merge preserves history on `main`).
- Reference issues using `Fixes #N` or `Closes #N`.

## Synchronization & Prohibitions
- Rebase branch on `origin/main` before opening or updating PRs.
- Never commit directly to `main`.
- Never force-push (`git push --force` or `--force-with-lease` are prohibited for agents).
