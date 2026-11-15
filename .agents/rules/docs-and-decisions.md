---
description: "Apply when editing docs, specs, or ADRs"
trigger: model_decision
---

# Documentation & Decisions

## Architecture Decision Records (ADRs)
- An accepted ADR in `docs/decisions/` is required before introducing:
  - New external crate dependencies.
  - State persistence or database engines.
  - Deployment architecture or container infrastructure changes.
  - Public interface or breaking bot behavior changes.

## Specifications (Optional for Small Tasks)
- Small/single-PR tasks use the GitHub Issue as their spec.
- Large multi-PR features use `docs/specs/NNN-slug/` (`draft` -> `approved` via merged PR -> `implemented`); if an issue links a spec, wait until it is approved.

## Documentation Style & Formatting
- Keep markdown files small and focused on one logical topic (no arbitrary line limit).
- Use relative links only between documents; avoid external links where local context suffices.
- Prefer Markdown tables and structured bullet lists over long prose paragraphs.
- Keep documentation dense, precise, and free of conversational filler.
