# Record Architecture Decisions

- Status: Accepted
- Date: 2026-11-13

## Context and Problem Statement

`botifil` requires architectural consistency across human maintainers and autonomous AI agents. Informal decision records in chat, pull request descriptions, or issue trackers quickly rot and leave agents without durable context for "why" constraints exist.

## Decision Drivers

- Maintain durable, version-controlled architectural rationale.
- Provide structured context easily ingested by AI coding agents.
- Keep overhead minimal and co-located with source code.

## Considered Options

- Markdown Architectural Decision Records (MADR 4) in `docs/decisions/`
- Ad-hoc markdown notes or README sections
- GitHub Wiki or external documentation tools

## Decision Outcome

Chosen option: MADR 4 in `docs/decisions/`. It provides standardized, compact templates that git tracks natively alongside code.

## Consequences

- Positive: Clear institutional memory and constraints for autonomous agents.
- Positive: Every non-trivial architectural choice is reviewed as a PR.
- Negative: Authors must maintain records and index tables when designs change.
