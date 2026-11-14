# Tasks: Join Request Gatekeeper

- Spec: [spec.md](./spec.md)
- Plan: [plan.md](./plan.md)

## Slices

### Task 1: Domain Models, Callback Codec & Error Classifier
- **Parent Spec/Issue**: docs/specs/001-join-request-gatekeeper/spec.md
- **Owned Files**:
  - `Cargo.toml`
  - `src/domain.rs`
  - `src/error.rs`
  - `src/lib.rs`
  - `src/main.rs`
- **Slice Boundary**: Arithmetic challenge models, callback codec (`v1:<token>:<answer>` <= 64 bytes), and 7-row Bot API error classifier.
- **Non-Goals**: Handlers, persistent store, network dispatcher.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 2: Policy Configuration
- **Parent Spec/Issue**: docs/specs/001-join-request-gatekeeper/spec.md
- **Owned Files**:
  - `src/config.rs`
- **Slice Boundary**: Policy config parsing for `BOTIFIL_ALLOWED_CHATS`, `BOTIFIL_CHALLENGE_TTL_SECS`, and `BOTIFIL_MAX_ATTEMPTS`.
- **Non-Goals**: Session state, network handlers, bio screening rules.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 3: In-Memory Bounded TTL Session Store & Reaper
- **Parent Spec/Issue**: docs/specs/001-join-request-gatekeeper/spec.md
- **Owned Files**:
  - `src/lib.rs`
  - `src/store.rs`
- **Slice Boundary**: Thread-safe bounded `RwLock` store with attempt decrement and TTL background eviction reaper.
- **Non-Goals**: External database engine, Telegram API interaction.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 4: Telegram Join-Request & Callback Handlers
- **Parent Spec/Issue**: docs/specs/001-join-request-gatekeeper/spec.md
- **Owned Files**:
  - `src/handlers/callback.rs`
  - `src/handlers/join_request.rs`
  - `src/handlers/mod.rs`
  - `src/lib.rs`
- **Slice Boundary**: Join request handler issuing challenge DM and callback handler checking answers with pre-resolution `editMessageText`.
- **Non-Goals**: Bio regex screening, standalone binary runner.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 5: Dispatcher Wiring, Expiry Reaper & WireMock E2E
- **Parent Spec/Issue**: docs/specs/001-join-request-gatekeeper/spec.md
- **Owned Files**:
  - `src/main.rs`
  - `tests/gatekeeper_e2e.rs`
- **Slice Boundary**: Dispatcher pipeline wiring with `Throttle`, background reaper task, and WireMock integration tests.
- **Non-Goals**: Production deployment packaging, bio regex inspection.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly

### Task 6: Bio Screening & Ops Documentation
- **Parent Spec/Issue**: docs/specs/001-join-request-gatekeeper/spec.md
- **Owned Files**:
  - `.env.example`
  - `README.md`
  - `src/handlers/join_request.rs`
  - `src/lib.rs`
  - `src/screening.rs`
- **Slice Boundary**: Synchronous applicant bio regex screening pre-filter, environment configuration templates, and operational documentation.
- **Non-Goals**: Multi-language NLP classification, external reputation APIs.
- **Verifier Command**: `scripts/verify.sh`
- **Definition of Done**:
  - [ ] Files stay <= 300 lines
  - [ ] Diff stays <= 300 LoC
  - [ ] Verifier passes cleanly
