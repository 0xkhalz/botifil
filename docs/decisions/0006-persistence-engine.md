# Persistence Engine for Session Storage

- Status: Proposed
- Date: 2026-11-14

## Context and Problem Statement

Botifil manages join request challenges and verification sessions. In M2, sessions reside in an in-memory `RwLock` store, losing state on bot restarts and crashing pending admissions. Milestone M3 requires durable, restart-safe persistence for pending join sessions on a memory-constrained host without introducing C runtime dependencies.

## Decision Drivers

- Zero C toolchain dependencies for reliable static `x86_64-unknown-linux-musl` builds ([ADR 0004](0004-static-musl-distroless-deploy.md)).
- Minimal RAM and binary overhead on constrained host / VPS ([ADR 0005](0005-rust-quality-and-low-ram-baseline.md)).
- ACID MVCC transactional durability across crashes and service restarts.
- Compatibility with `deny.toml` license constraints and zero-panic error handling.

## Considered Options

- `redb` (embedded pure-Rust transactional key-value store)
- `rusqlite` (embedded SQLite requiring C toolchain and musl linking workarounds)
- `sqlx` with PostgreSQL or SQLite (async connection pool overhead and external services)

## Decision Outcome

Chosen option: `redb`. It is pure Rust, requires no C compiler or foreign bindings, builds cleanly for static musl targets, supports ACID transactions with MVCC, and has a minimal memory footprint suitable for single-file embedded key-value storage.

## Consequences

- Positive: Zero C toolchain dependencies for static musl compilation in distroless containers.
- Positive: ACID transactions prevent session corruption during abrupt process termination.
- Positive: Low memory and disk footprint with embedded single-file storage.
- Negative: Key-value table semantics require manual serialization instead of relational SQL queries.
