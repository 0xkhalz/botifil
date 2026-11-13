# Static Musl Distroless Deployment

- Status: Accepted
- Date: 2026-11-13

## Context and Problem Statement

Production deployments on a VPS require minimal attack surface, low memory overhead, reproducible container artifacts, and secure non-root execution without depending on host runtime libraries.

## Decision Drivers

- Minimize container image footprint and runtime RAM usage.
- Eliminate unnecessary binaries, package managers, and shells from the image.
- Enforce non-root execution by default.
- Enable deterministic builds via multi-stage layer caching.

## Considered Options

- Static `x86_64-unknown-linux-musl` binary in `gcr.io/distroless/static-debian12:nonroot`
- Dynamic glibc binary in Debian or Ubuntu slim container
- Alpine Linux container image with musl libc and busybox shell
- Bare binary running as a systemd service directly on the host

## Decision Outcome

Chosen option: Static `x86_64-unknown-linux-musl` binary packaged inside `gcr.io/distroless/static-debian12:nonroot` deployed via `podman`. Multi-stage caching will be handled with `cargo-chef` in milestone M4.

## Consequences

- Positive: Zero system shell or package manager in the production image.
- Positive: Minimal image size (<30MB) and hardened non-root container user.
- Negative: Musl cross-compilation requires static linking of any C dependencies.
