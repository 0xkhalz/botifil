# Use Rust and Teloxide Framework

- Status: Accepted
- Date: 2026-11-13

## Context and Problem Statement

Telegram join request bots handle sensitive user admission workflows, strict Bot API rate limits, and asynchronous challenge states. We need a reliable language and framework foundation that guarantees memory safety and strong domain typing.

## Decision Drivers

- Compile-time distinction between Telegram entities (`ChatId` vs `UserId`).
- Built-in Telegram Bot API rate-limiting and backoff support.
- Expressive finite state machine (FSM) abstractions for challenge flows.
- Predictable runtime resource consumption under resource constraints.

## Considered Options

- Rust (Edition 2024) with `teloxide` 0.17 (`macros`, `throttle`, later `webhooks-axum`)
- Rust with `teloxide-core` minimal client only
- Go with `gotgbot/v2` or `go-telegram/bot`

## Decision Outcome

Chosen option: Rust (Edition 2024) with `teloxide` 0.17. It enforces strong type safety between `ChatId` and `UserId`, provides the `Throttle` adaptor out of the box, and offers `dptree` for FSM challenge handling. Compile-time RAM overhead is mitigated via `[profile.dev]` settings in [ADR 0005](0005-rust-quality-and-low-ram-baseline.md).

## Consequences

- Positive: Eliminates whole classes of runtime routing bugs through type safety.
- Positive: Automatic rate-limit handling prevents Telegram 429 penalties.
- Negative: Higher initial compilation time and binary build memory than Go.
