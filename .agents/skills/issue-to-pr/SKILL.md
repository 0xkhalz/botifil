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
   - When dispatched by an orchestrator, use the orchestrator-provisioned worktree; otherwise create a flat worktree `.worktrees/<n>-<slug>` for branch `<type>/<n>-<slug>` per [.agents/rules/git-workflow.md](../../../.agents/rules/git-workflow.md):
     ```bash
     git fetch origin && git worktree add .worktrees/<n>-<slug> -b <type>/<n>-<slug> origin/main
     cd .worktrees/<n>-<slug>
     ```

4. **TDD Implementation Loop & Continuous Atomic Commits**:
   - Modify only assigned owned files and enforce [AGENTS.md](../../../AGENTS.md) invariants (no `unwrap`/`expect`/`panic!` outside tests, no `unsafe`, preserve newtypes).
   - Follow red-green-refactor and commit **continuously at each logical change boundary** (no end-of-task dump commits):
     - Commit preparatory `refactor:` changes first with tests passing.
     - Keep unit/regression tests with their `feat:`/`fix:` slice; commit standalone test additions/refactors as `test:`.
     - Stage explicit paths or hunks (`git add <path1> <path2>` or `git add -p`, never `git add -A` or `git add .`), inspect `git diff --cached`, and use change-based Conventional Commit messages (never `--no-verify`).

5. **Run Verification Pipeline**:
   - Execute verification suite (and re-run after any final refactor or fix):
     ```bash
     ./scripts/verify.sh
     ```
   - Resolve errors until exit 0. After 3 consecutive failed repair attempts, escalate with `status:needs-human` and stop.

6. **Pre-Push Audit & Rebase**:
   - Inspect `git log --stat origin/main..HEAD` to confirm atomic, change-based commits and no transient/non-essential files.
   - Rebase on `origin/main` (resolve only session-modified files; abort and ask the user on foreign conflicts):
     ```bash
     git fetch origin && git rebase origin/main
     ```

7. **Push & Open Pull Request**:
   - Push branch to remote:
     ```bash
     git push -u origin <type>/<n>-<slug>
     ```
   - Ensure one logical scope per PR and open PR using [.github/PULL_REQUEST_TEMPLATE.md](../../../.github/PULL_REQUEST_TEMPLATE.md) with a Conventional Commit `--title` and `--body-file <tmpfile>` containing `Fixes #<n>` (repeat keyword for multiple: `Fixes #1, Fixes #2`), diff confirmation (`<= 300` LoC), and `./scripts/verify.sh` tail:
     ```bash
     gh pr create --base main --title "<type>: <concise summary>" --body-file <tmpfile>
     ```

8. **Watch CI & Resolve Reviews**:
   - Monitor checks: `gh pr checks --watch`.
   - When reviewer comments arrive, commit fixes atomically at each logical change boundary, re-verify with `./scripts/verify.sh`, push commits, reply via `--body-file` or `-F` (never inline strings), and resolve threads.
   - Never run `gh pr review --approve` or `gh pr merge`.

9. **Cleanup Worktree**:
   - After maintainer rebase-merges PR, delete worktree and branch:
     ```bash
     cd ../.. && git worktree remove .worktrees/<n>-<slug> && git branch -d <type>/<n>-<slug>
     ```
