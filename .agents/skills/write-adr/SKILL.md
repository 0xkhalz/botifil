---
name: write-adr
description: Propose and record Architecture Decision Records using MADR 4 in docs/decisions/
---

# Write ADR

Workflow for proposing and recording architectural decisions in `botifil`.

## Trigger
- Proposing a new external dependency or crate.
- Introducing or altering state persistence engines.
- Modifying deployment architecture, runtime environments, or container specs.
- Making architectural or structural changes to bot design.

## Steps
1. **Find Next Number**: Check [docs/decisions](../../../docs/decisions/README.md) for the next sequence `NNNN` (e.g. `0006`).
2. **Copy Template**: Copy [assets/madr.md](./assets/madr.md) to `docs/decisions/NNNN-slug.md`.
3. **Fill MADR Sections**:
   - Set title and date (YYYY-MM-DD); keep status as `Proposed`.
   - Provide problem context, drivers, and at least two considered options.
   - Articulate chosen outcome with explicit positive and negative consequences.
4. **Update Decision Index**: Add the new ADR row to the table in [docs/decisions/README.md](../../../docs/decisions/README.md).
5. **Verify**: Run `scripts/verify.sh` to validate formatting, line ceilings, and links.
6. **Open PR**: Submit a pull request titled `docs(adr): propose NNNN slug`.
