# Telegram WebApp Verification UI

- Status: Proposed
- Date: 2026-11-15

## Context and Problem Statement

Milestone M5 introduces an interactive Telegram Mini App (`WebApp`) verification UI for join-request screening. Telegram does not allow bots to open a modal directly when a user triggers a native "Request to Join" (`ChatJoinRequest`). Instead, `botifil` sends an ephemeral direct message with an `InlineKeyboardButton::web_app` button that launches a `WebApp` served by the `botifil` binary behind external TLS termination.

Serving and validating this `WebApp` requires an HTTP/1.1 endpoint, subpath-aware routing, UI delivery, and cryptographic verification of Telegram's signed `initData` payload (`WebAppData` HMAC-SHA256) without violating `botifil`'s low-resource and dependency constraints.

## Decision Drivers

- Low RAM and compile-time footprint on constrained hardware (7.6 GiB RAM, `CARGO_BUILD_JOBS=2`), per [ADR 0005](0005-rust-quality-and-low-ram-baseline.md).
- Static `x86_64-unknown-linux-musl` distroless container deployment with zero C runtime dependencies, per [ADR 0004](0004-static-musl-distroless-deploy.md).
- Strict safety and dependency discipline: `unsafe_code = "forbid"`, zero panics (`unwrap`, `expect`, `panic!` denied), and `deny.toml` compliance ([ADR 0005](0005-rust-quality-and-low-ram-baseline.md)).
- Direct integration with Teloxide / Tokio ([ADR 0002](0002-rust-teloxide-framework.md)) and embedded `redb` session persistence ([ADR 0006](0006-persistence-engine.md)).

## Considered Options

- **Option 1 — Embedded `tokio::net::TcpListener` HTTP/1.1 server + pure-Rust `initData` verifier**: Serve a self-contained Rose-Pine HTML/CSS/JS WebApp UI over `tokio::net::TcpListener` (`tokio` features `net` and `io-util`), validate `initData` using pure-Rust RFC 2104 HMAC-SHA256 / FIPS 180-4 SHA-256 with constant-time comparison, and promote `url = "2"` (already locked in `Cargo.lock`) from `[dev-dependencies]` to `[dependencies]`.
- **Option 2 — External web framework (`axum` + `tower-http` + `hmac` + `sha2`)**: Introduces a multi-crate HTTP middleware and cryptography stack, increasing dependency surface, compilation RAM/time, and binary footprint.
- **Option 3 — Inline DM callback buttons only or separate static SPA host**:
  - Inline DM callback buttons only: avoids an HTTP listener, but cannot deliver the interactive M5 `WebApp` UX.
  - Separate static SPA host: splits deployment across services and requires CORS and cross-process HMAC secret distribution.

## Decision Outcome

Chosen option: **Option 1 — Embedded `tokio::net::TcpListener` HTTP/1.1 server + pure-Rust `initData` verifier**.

Enabling `tokio` `net` + `io-util` and promoting `url = "2"` (already present in `Cargo.lock`) introduces zero new transitive crates to `Cargo.lock` while keeping compilation memory, binary size, and musl cross-builds minimal. Pure-Rust FIPS 180-4 SHA-256 and RFC 2104 HMAC-SHA256 satisfy `unsafe_code = "forbid"` and avoid extra crypto dependencies, while in-process HTTP serving gives direct access to `redb` session state ([ADR 0006](0006-persistence-engine.md)) and Teloxide actions ([ADR 0002](0002-rust-teloxide-framework.md)).

## Consequences

- Positive: Single static musl binary and distroless container deployment with zero CORS or cross-service secret distribution.
- Positive: Zero new transitive crates added to `Cargo.lock`; only `url = "2"` promotion and `tokio` `net` + `io-util` features are enabled.
- Positive: Low compile-time and runtime RAM overhead with `unsafe_code = "forbid"` preserved.
- Positive: Constant-time `WebAppData` HMAC-SHA256 verification prevents forged `initData` submissions.
- Negative: Production deployments require a TLS-terminating reverse proxy or tunnel in front of `botifil` because Telegram enforces HTTPS `WebApp` URLs.
- Negative: Minimal custom HTTP/1.1 handling demands strict request-line, header (`chunked` / duplicate `Content-Length`), payload-size, timeout, and `EMFILE` accept-backoff handling.
- Negative: In-repo SHA-256 / HMAC-SHA256 code requires dedicated RFC 4231 and Telegram `WebAppData` test-vector coverage.
