---
description: "Apply when writing, editing, or reviewing Rust code"
trigger: model_decision
---

# Rust Coding Standards

## Error Handling & Types
- Use `thiserror` for domain and module error enums with actionable variants.
- Reserve `anyhow` strictly for top-level application orchestration in `main.rs`.
- Preserve teloxide `ChatId` and `UserId` newtypes; never downgrade to raw `i64` for Telegram API calls.
- No `unwrap()`, `expect()`, or `panic!` outside unit tests; `unsafe` code is strictly prohibited.

## Concurrency & Async
- Never invoke blocking operations inside async tasks; use `tokio::task::spawn_blocking` if unavoidable.
- Prefer immutable data structures and return new values over in-place mutation.
- Wrap bot instances with teloxide's `Throttle` adaptor to respect Telegram rate limits.

## Logging & Testing
- Use `tracing` (`info!`, `warn!`, `error!`, `debug!`) instead of `println!` or `eprintln!`.
- Write unit tests for pure logic.
- Use `wiremock` for mocking Telegram Bot API HTTP responses in integration tests.
