---
name: write-spec
description: Author and review specification packets in docs/specs/
---

# Write Spec

Workflow for authoring and reviewing feature specifications in `botifil`.

## Trigger
- Starting a new feature, milestone, or task labeled `status:needs-spec`.
- Multi-step functionality requiring architectural consensus before coding.

## Steps
1. **Determine Directory**: Check [docs/specs](../../../docs/specs/README.md) for the next sequence `NNN` (e.g. `001-join-requests`).
2. **Scaffold Files**: Create `docs/specs/NNN-slug/` and copy the templates:
   - [spec.md](./assets/spec.md) to `docs/specs/NNN-slug/spec.md`
   - [plan.md](./assets/plan.md) to `docs/specs/NNN-slug/plan.md`
   - [tasks.md](./assets/tasks.md) to `docs/specs/NNN-slug/tasks.md`
3. **Specify Requirements**: Fill requirements, user-facing behavior, and interfaces in `spec.md` with status `draft`.
4. **Detail Architecture**: In `plan.md`, document component design, data flows, and failure modes.
5. **Decompose Tasks**: In `tasks.md`, define granular tasks where each task:
   - Maps 1:1 to `.github/ISSUE_TEMPLATE/task.yml`.
   - Specifies an explicit owned-files list, slice boundary, non-goals, and verifier command.
   - Strictly obeys the <= 300 LoC PR diff limit.
6. **Verify and Submit**: Run `scripts/verify.sh` to validate formatting and links. Open a PR titled `docs(spec): add NNN-slug specification`.
7. **Lifecycle Progression**: Merging the spec PR marks the specification `approved` and authorizes creating task issues.
