# Roadmap

Project milestones for `botifil` development and delivery.

| Milestone | Scope | Status |
| --- | --- | --- |
| M0 | Bootstrap governance, CI, rules, and skills | In progress |
| M1 | Walking skeleton: Cargo crate, `getMe`, `tracing`, graceful shutdown | Planned |
| M2 | Join-request gatekeeper: `chat_join_request` -> DM challenge -> approve/decline | Planned |
| M3 | Persistence and restart safety (`redb` vs SQLite, opens `ADR 0006`) | Planned |
| M4 | Containerfile (`cargo-chef` + musl + distroless) + VPS podman deploy | Planned |
| M5 | Webhooks (`webhooks-axum`, optional) | Deferred |

## Execution Principles

- Every milestone ships via small, atomic pull requests (<= 300 LoC).
- Architecture decisions are captured in ADRs before implementation.
- Verifications run locally via `scripts/verify.sh` and remotely in CI.
