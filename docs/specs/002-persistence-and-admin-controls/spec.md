# Specification: Persistence and Admin Controls

- Status: draft
- Date: 2026-11-14
- Milestone: M3 & M4

## Motivation & Context

In-memory sessions in M2 do not survive process restarts. Milestone M3 introduces durable session persistence using an embedded pure-Rust store ([ADR 0006](../../decisions/0006-persistence-engine.md)). Milestone M4 packages the application into a hardened, static musl distroless container for unprivileged deployment ([ADR 0004](../../decisions/0004-static-musl-distroless-deploy.md)).

## User-Facing Behavior

1. **Restart Resiliency**: Applicants with active join challenges can complete verification after a bot restart or upgrade without re-requesting.
2. **Expired Challenge Cleanup**: Challenges expired during downtime are purged on startup, leaving Telegram requests pending for manual admin review.
3. **Containerized Execution**: Bot runs as an unprivileged container with read-only rootfs and a mounted data volume for the database.

## Technical Requirements

### Session Durability & Storage (M3)
- Engine: Embedded `redb` key-value database stored in `BOTIFIL_DB_PATH` (default: `./data/botifil.redb`).
- Schema: Table storing `SessionToken` -> serialized `PendingJoin` records.
- Expiry Recovery: Records store wall-clock expiration epoch (`u64` seconds). Upon startup, a reconciliation pass evicts expired records and schedules in-memory timers for unexpired entries.
- Trait Abstraction: Handlers interact with an asynchronous or sync `Storage` trait to allow mock implementations in tests.

### Containerization & Deployment (M4)
- Multi-Stage Build: `cargo-chef` recipe caching dependencies before compiling for `x86_64-unknown-linux-musl`.
- Base Image: `gcr.io/distroless/static-debian12:nonroot` running as UID `65532:65532`.
- Filesystem: Root filesystem is read-only; `/data` directory is mounted with write permissions for `redb`.
- Orchestration: Validated using `podman` with drop-all Linux capabilities (`--cap-drop=ALL`).

## Out of Scope

- Relational database engines (PostgreSQL, SQLite) or network database services.
- Multi-instance distributed coordination (single-replica primary).
- Web dashboard or UI admin panels.

## Verification & Acceptance

- Integration tests verify `PendingJoin` survival and recovery across store close and reopen cycles.
- Multi-stage container builds reproducibly with static musl and passes security scanner checks.
- Verification command `./scripts/verify.sh` succeeds with zero errors.
