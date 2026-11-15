---
name: issue-to-pr
description: Implements an assigned GitHub issue through an isolated worktree, TDD loop, automated verification, and conventional pull request.
---

# Issue to PR

Executes the implementation lifecycle for an assigned issue in an isolated worktree.

## Trigger

Activated when assigned a GitHub issue to implement.

## Workflow Steps

1. **Inspect Issue & Scope**:
   - Inspect issue details: `gh issue view <n>`.
   - Verify label `status:ready`. If labeled `status:needs-spec` or scope is ambiguous, stop and escalate:
     ```bash
     gh issue edit <n> --add-label status:needs-human
     ```
   - Read linked specifications in [docs/specs/README.md](../../../docs/specs/README.md) or architecture decisions in [docs/decisions/README.md](../../../docs/decisions/README.md).

2. **Claim Issue**:
   - Transition issue label to in-progress:
     ```bash
     gh issue edit <n> --remove-label status:ready --add-label status:in-progress
     ```

3. **Provision Isolated Worktree**:
   - Create branch `<type>/<n>-<slug>` in dedicated worktree per [.agents/rules/git-workflow.md](../../../.agents/rules/git-workflow.md):
     ```bash
     git fetch origin && git worktree add .worktrees/<branch> -b <type>/<n>-<slug> origin/main
     cd .worktrees/<branch>
     ```

4. **TDD Implementation Loop**:
   - Modify only assigned owned files.
   - Follow red-green-refactor cycle: write failing test first, implement minimal code, verify green.
   - Enforce [AGENTS.md](../../../AGENTS.md) invariants (no `unwrap`/`expect`/`panic!` outside tests, no `unsafe`, preserve newtypes).

5. **Run Verification Pipeline**:
   - Execute verification suite:
     ```bash
     ./scripts/verify.sh
     ```
   - Resolve errors until exit 0. After 3 consecutive failed repair attempts, escalate with `status:needs-human` and stop.

6. **Atomic Conventional Commits (One Job = One Commit)**:
   - Enforce **one job per commit**: each commit must represent one single logical job or concern (never bundle unrelated edits, even 1-line changes in different concerns). `<= 300 LoC` is the PR diff ceiling, not a single-commit target.
   - Stage one job at a time using sequential explicit paths or `git add -p` (never `git add -A` or `git add .`):
     ```bash
     git add <path1> <path2>   # or: git add -p <path>
     git diff --cached         # verify only one job is staged
     ```
   - Commit each job separately using Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `chore:`). Never use `--no-verify`.
   - Before pushing, inspect `git log --stat origin/main..HEAD` to confirm one clean commit per job.

7. **Push & Open Pull Request**:
   - Push branch to remote:
     ```bash
     git push -u origin <type>/<n>-<slug>
     ```
   - Open PR using [.github/PULL_REQUEST_TEMPLATE.md](../../../.github/PULL_REQUEST_TEMPLATE.md) containing `Fixes #<n>`, diff confirmation (<= 300 LoC), and `./scripts/verify.sh` tail:
     ```bash
     gh pr create --base main --body-file <tmpfile>
     ```

8. **Watch CI & Resolve Reviews**:
   - Monitor checks: `gh pr checks --watch`.
   - When reviewer comments arrive, fix code, re-verify with `./scripts/verify.sh`, push commits, reply to feedback, and resolve threads.
   - Never run `gh pr review --approve` or `gh pr merge`.

9. **Cleanup Worktree**:
   - After maintainer rebase-merges PR, delete worktree and branch:
     ```bash
     cd ../.. && git worktree remove .worktrees/<branch> && git branch -d <type>/<n>-<slug>
     ```
