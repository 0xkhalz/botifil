---
name: review-pr
description: Conducts an independent, fresh-context code review on an open pull request and submits thread-blocking comments.
---

# Review PR

Performs automated code and quality reviews on open pull requests.

## Trigger & Constraints

- **Trigger**: Activated when assigned to review an open pull request.
- **Constraints**: Reviewers operate with read/comment permissions only. Reviewers must never edit code, push commits, execute `gh pr review --approve`, or merge pull requests (`gh pr merge`).

## Workflow Steps

1. **Inspect Pull Request**:
   - Inspect PR metadata, diff, and checks non-destructively:
     ```bash
     gh pr view <n>
     gh pr diff <n>
     gh pr checks <n>
     ```

2. **Verify PR Shape & Commit Discipline**:
   - Verify diff ceiling: `git diff --stat origin/main` <= 300 LoC (excluding `Cargo.lock`).
   - Verify file ceiling: every modified file <= 300 lines.
   - Verify commit history follows Conventional Commits (rebase-merge preserves history on `main`).
   - Verify touched files stay strictly within owned scope from the linked issue.

3. **Audit Code Quality & Invariants**:
   - **Rust Standards**: No `unwrap()`, `expect()`, or `panic!` outside tests; zero `unsafe`; preserve newtypes per [.agents/rules/rust-code.md](../../../.agents/rules/rust-code.md).
   - **Dependencies**: No unauthorized crates added without an accepted ADR in [docs/decisions/README.md](../../../docs/decisions/README.md).
   - **Security**: No tokens, group/user IDs, or personal paths; callback data <= 64 bytes per [.agents/rules/security.md](../../../.agents/rules/security.md).
   - **Verification**: Ensure PR body contains `./scripts/verify.sh` tail ending with `verify: OK`.

4. **Submit Review Summary**:
   - Post review summary comment via temporary file:
     ```bash
     gh pr review <n> --comment --body-file <tmpfile>
     ```
   - Must start with explicit `Verdict: NICE` or `Verdict: CHANGES`.

5. **Post Inline Thread Comments**:
   - For every CRITICAL, HIGH, or MEDIUM finding, post an inline review comment to ensure thread resolution blocks merge:
     ```bash
     gh api -X POST "repos/{owner}/{repo}/pulls/<n>/comments" \
       -f body="[SEVERITY] Detailed description and suggested fix" \
       -f commit_id="<head_sha>" \
       -f path="<file_path>" \
       -F line=<line_number>
     ```
