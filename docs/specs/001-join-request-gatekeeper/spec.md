# Specification: Join Request Gatekeeper

- Status: draft
- Date: 2026-11-14
- Milestone: M2

## Motivation & Context

Automate Telegram supergroup/channel join request admission using interactive arithmetic CAPTCHAs. Refer to [Telegram Join Request Domain](../../domain/telegram-join-requests.md) for Bot API mechanics and lifecycle constraints.

## User-Facing Behavior

1. **Join Request Trigger**: When applicant requests to join, bot initiates an ephemeral DM.
2. **Arithmetic Challenge**: DM presents `A + B = ?` (`1 <= A, B <= 9`) with 3 inline buttons (1 correct, 2 distractors).
3. **Attempt Policy**: Configurable `max_attempts` (default `2`, valid range `1..=5`). Wrong answers decrement attempts and prompt retry.
4. **Resolution Feedback**: Bot edits DM (`editMessageText`) to "Approved" or "Declined" before resolving join request.

## Technical Requirements

### Routing Invariants & Identifiers
- DM operations: `sendMessage` and `editMessageText` MUST target `req.user_chat_id`.
- Join resolution: `approveChatJoinRequest` and `declineChatJoinRequest` MUST target `req.chat.id` with `req.from.id`.
- Edit ordering: `editMessageText` MUST precede approve/decline calls before the ephemeral ticket expires.

### Callback Data Encoding
- Format: `v1:<token>:<answer>` (strictly <= 64 UTF-8 bytes).
- `<token>`: 16-hex-char standard-library session identifier (zero external crypto/random crates).
- State: In-memory `tokio::sync::RwLock` session store with zero new external crates.

### Admission & Decline Policies
- **Auto-Approve**: Issued on correct answer callback.
- **Auto-Decline**: Issued ONLY on exhausted wrong attempts or bio pre-screening rejection.
- **Leave Pending for Admins**: On 300s TTL expiry or `403 Forbidden` (`USER_IS_BLOCKED`, `bot can't initiate conversation`), evict local session, log non-PII outcome, and leave request pending in Telegram for manual admin review.

### Configuration
- `BOTIFIL_ALLOWED_CHATS`: Optional comma-separated `ChatId` allowlist.
- `BOTIFIL_CHALLENGE_TTL_SECS`: Challenge lifespan in seconds (default `300`, max `300`).

## Out of Scope
- Persistent database storage (in-memory `RwLock` only).
- Webhook server deployment (long polling dispatcher in M2).
- Multilingual i18n localization (English-only text).

## Verification & Acceptance
- Unit tests verify callback codec (`<= 64` bytes) and arithmetic generator.
- Integration tests simulate join requests, callbacks, retries, and timeouts.
- `./scripts/verify.sh` passes cleanly (exit code 0).
