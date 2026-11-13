# Rust Quality and Low-RAM Baseline

- Status: Accepted
- Date: 2026-11-13

## Context and Problem Statement

To prevent memory bloat, undefined behavior, and unhandled panics, `botifil` requires strict compiler lints, low-RAM dev profiles for constrained build environments, and standardized crate dependencies starting in milestone M1.

## Decision Drivers

- Forbid unsafe code and deny panic-prone unwraps across production crates.
- Reduce compiler RAM consumption on memory-constrained systems.
- Establish standard error handling, diagnostics, and testing toolchains.

## Considered Options

- Strict workspace lints with low-RAM dev profile and thin LTO release profile
- Default Rust compiler and Clippy configurations without strict denies

## Decision Outcome

Chosen option: Apply the following baseline verbatim in `Cargo.toml` during M1:

- `[lints.rust]` `unsafe_code = "forbid"`
- `[lints.clippy]` `pedantic = { level = "warn", priority = -1 }`, `unwrap_used = "deny"`, `expect_used = "deny"`, `panic = "deny"`, `module_name_repetitions = "allow"`, `must_use_candidate = "allow"`
- `[profile.dev]` `codegen-units = 1`, `debug = "line-tables-only"`, `incremental = false`
- `[profile.release]` `lto = "thin"`, `strip = true`, `panic = "abort"`
- Baseline dependencies: `tracing` + `tracing-subscriber`, `thiserror` + `anyhow`, `cargo-nextest`, `cargo-deny`, `cargo-shear`.

State persistence (`redb` vs SQLite) is explicitly deferred to `ADR 0006`.

## Consequences

- Positive: Zero unsafe code and eliminated runtime panic sources.
- Positive: Dev builds consume significantly less peak RAM.
- Negative: Strict clippy denies require explicit error propagation handling.
