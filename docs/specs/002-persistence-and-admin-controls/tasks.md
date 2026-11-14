# Tasks: Persistence and Admin Controls

- Spec: [spec.md](./spec.md)
- Plan: [plan.md](./plan.md)

## Slices

### Task 1: M3-1 Storage Trait & Redb Session Store
- **Parent Spec/Issue**: docs/specs/002-persistence-and-admin-controls/spec.md
- **Owned Files**:
  - `Cargo.toml`
  - `src/store/memory.rs`
  - `src/store/mod.rs`
  - `src/store/redb.rs`
- **Slice Boundary**: `Storage` trait definition and `RedbSessionStore` implementation using `redb`.
- **Non-Goals**: Handlers wiring, container packaging.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 2: M3-2 Startup Expiry Reconciliation & Dispatcher Wiring
- **Parent Spec/Issue**: docs/specs/002-persistence-and-admin-controls/spec.md
- **Owned Files**:
  - `src/config.rs`
  - `src/main.rs`
  - `src/store/reconciliation.rs`
  - `tests/persistence_test.rs`
- **Slice Boundary**: Startup reconciliation pass, TTL expiry timer rearming, and store configuration.
- **Non-Goals**: Containerization, multi-host sync.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 3: M4-1 Multi-Stage Static Musl Containerfile
- **Parent Spec/Issue**: docs/specs/002-persistence-and-admin-controls/spec.md
- **Owned Files**:
  - `.dockerignore`
  - `Containerfile`
- **Slice Boundary**: `cargo-chef` multi-stage build producing static musl binary in distroless image.
- **Non-Goals**: Rust code changes, deployment scripting.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 4: M4-2 Podman Deployment Harness & Ops Docs
- **Parent Spec/Issue**: docs/specs/002-persistence-and-admin-controls/spec.md
- **Owned Files**:
  - `README.md`
  - `scripts/deploy-podman.sh`
- **Slice Boundary**: Podman run harness with `--cap-drop=ALL`, volume permissions, and ops documentation.
- **Non-Goals**: Cloud orchestration, CI pipeline modifications.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly
