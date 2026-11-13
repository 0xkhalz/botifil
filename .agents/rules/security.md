---
description: "Security invariants, token handling, input validation, and GitHub Actions hardening"
trigger: always_on
---

# Security Rules

## Secrets & Identifiers
- Telegram bot token must be passed solely via `TELOXIDE_TOKEN` environment variable.
- Redact secrets in logs: implement custom `Debug` or avoid deriving `Debug` on config/secret types.
- Never commit, print, or log bot tokens, group IDs, user IDs, or personal file paths.
- `gitleaks` must pass cleanly locally and in CI.

## Input Validation
- Validate all untrusted Telegram input at system boundaries.
- Limit inline keyboard `callback_data` to ≤ 64 UTF-8 bytes; protect state with HMAC or opaque tokens.
- Enforce strict typing on Telegram `ChatId` and `UserId` to avoid confusion or injection.

## CI & GitHub Actions Hardening
- Workflows must declare least-privilege `permissions: contents: read` at top level.
- Pin third-party GitHub Actions to immutable 40-character commit SHAs.
- Never expose repository secrets in pull requests from forks or untrusted branches.
