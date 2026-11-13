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

## Specifications Lifecycle
- Features must define a specification in `docs/specs/`.
- Lifecycle status progression: `draft` -> `approved` (via merged pull request) -> `implemented`.
- Agents only implement features whose spec is marked `approved`.

## Documentation Style & Formatting
- All markdown files must remain ≤ 300 lines total.
- Use relative links only between documents; avoid external links where local context suffices.
- Prefer Markdown tables and structured bullet lists over long prose paragraphs.
- Keep documentation dense, precise, and free of conversational filler.
