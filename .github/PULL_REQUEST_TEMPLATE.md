## Linked Issue

Fixes #

## Summary

<!-- What was changed and why -->

## Scope

- Files touched:
- Diff confirmation (`git diff --stat origin/main` <= 300 LoC):

## Verification Evidence

<!-- Paste tail of `scripts/verify.sh` -->
```text
verify: OK
```

## Checklist

- [ ] `scripts/verify.sh` passed (exit code 0)
- [ ] One logical scope per PR (no unrelated features, fixes, or refactors)
- [ ] Total diff `<= 300` LoC and every file `<= 300` lines
- [ ] One job per commit (no mixed change types; change-based commit messages; tests with their slice)
- [ ] Spec or ADR linked if applicable
- [ ] No secrets, tokens, private IDs, or local paths
- [ ] No unapproved dependencies added
