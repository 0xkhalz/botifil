---
name: telegram-join-requests
description: Use when implementing, modifying, or reviewing Rust code that handles Telegram join requests, verification DMs, inline callbacks, invite links, or approval/decline calls.
---

# Telegram Join Requests

Domain guide and checklist for Telegram Bot API join request processing in Rust with `teloxide`.

## Source of Truth

Refer to [telegram-join-requests.md](../../../docs/domain/telegram-join-requests.md) for full protocol specifications, sequence diagrams, and error handling matrices.

## Pre-Code & Pre-Review Checklist

Verify every implementation and code review against these domain invariants:

1. **Routing Identifiers & Types**:
   - `req.chat.id` (`ChatId`): Target group or channel identifier; use only for `approveChatJoinRequest` and `declineChatJoinRequest`.
   - `req.from.id` (`UserId`): Applicant user account; use only as `user_id` in join approval/decline methods.
   - `req.user_chat_id` (`ChatId`): Ephemeral 1-on-1 DM channel; use as `chat_id` for `sendMessage` and `editMessageText`. Never pass to approval methods.
2. **Ephemeral 5-Minute DM Window**:
   - Telegram grants a 300-second direct messaging ticket bound to `user_chat_id` from `req.date`.
   - Edit or update challenge UI before or concurrently with `approveChatJoinRequest` or `declineChatJoinRequest`; resolution calls immediately terminate the ticket.
3. **Update Delivery Whitelist**:
   - Polling and webhook `allowed_updates` must explicitly include `chat_join_request` and `callback_query` (plus `chat_member` for status transitions).
4. **Chat Administrator Rights**:
   - Bot must hold `can_invite_users` administrator permission in the target group or channel.
5. **Invite Link Configuration**:
   - `creates_join_request = true` is mutually exclusive with `member_limit`. Combining both fails with `400 Bad Request: MEMBER_LIMIT_INVALID`.
6. **Callback Data Ceiling**:
   - `InlineKeyboardButton.callback_data` ceiling is strictly 64 UTF-8 bytes. Truncate digest payloads or store session state in memory.
7. **Idempotent Error Handling**:
   - `HIDE_REQUESTER_MISSING`: Request resolved or canceled elsewhere; treat as idempotent no-op and clean up session.
   - `USER_ALREADY_PARTICIPANT`: User joined via alternative route; treat as idempotent success and evict local session.
8. **Rate Limiting & Throttling**:
   - Wrap client in `teloxide::adaptors::Throttle` to queue calls and respect group admin (20-30/min) and DM (30/sec) limits and `retry_after`.
