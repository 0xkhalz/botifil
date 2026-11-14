use std::sync::Arc;
use teloxide::{
    RequestError,
    payloads::SendMessageSetters,
    requests::Requester,
    types::{ChatJoinRequest, InlineKeyboardButton, InlineKeyboardMarkup},
};

use crate::{
    config::Config,
    domain::{CallbackAction, Challenge, DomainError, PendingJoin, SessionOutcome, SessionToken},
    error::{BotErrorAction, classify_request_error},
    store::{SessionStore, StoreError},
};

/// Builds the inline keyboard with choice buttons for the math challenge.
///
/// # Errors
/// Returns [`DomainError::CallbackTooLong`] if the encoded callback payload exceeds 64 bytes.
pub fn build_challenge_keyboard(
    token: &SessionToken,
    [c1, c2, c3]: [u8; 3],
) -> Result<InlineKeyboardMarkup, DomainError> {
    let btn = |c| {
        let act = CallbackAction::new(token.clone(), c);
        Ok(InlineKeyboardButton::callback(c.to_string(), act.encode()?))
    };
    Ok(InlineKeyboardMarkup::new([[btn(c1)?, btn(c2)?, btn(c3)?]]))
}

/// Handles an incoming chat join request update by issuing a verification challenge.
///
/// # Errors
/// Returns [`RequestError`] if sending the challenge DM fails with an unhandled Telegram API error.
pub async fn handle_join_request<B>(
    bot: B,
    req: ChatJoinRequest,
    config: Arc<Config>,
    store: SessionStore,
) -> Result<SessionOutcome, RequestError>
where
    B: Requester<Err = RequestError>,
{
    if !config.is_chat_allowed(req.chat.id) {
        return Ok(SessionOutcome::IdempotentNoOp);
    }
    let (token, challenge, now) = (
        SessionToken::generate(),
        Challenge::generate(),
        tokio::time::Instant::now(),
    );
    let Ok(kb) = build_challenge_keyboard(&token, challenge.choices) else {
        tracing::warn!("Failed to build challenge keyboard; skipping request");
        return Ok(SessionOutcome::IdempotentNoOp);
    };
    let pending = PendingJoin {
        token: token.clone(),
        chat_id: req.chat.id,
        user_id: req.from.id,
        user_chat_id: req.user_chat_id,
        challenge: challenge.clone(),
        attempts: 0,
        max_attempts: config.max_attempts(),
        message_id: None,
        expires_at: now + config.challenge_ttl(),
    };
    if let Err(StoreError::CapacityExceeded) = store.upsert(pending, now).await {
        tracing::warn!("Session store capacity exceeded; dropping join request");
        return Ok(SessionOutcome::IdempotentNoOp);
    }
    let res = bot
        .send_message(req.user_chat_id, challenge.prompt_text())
        .reply_markup(kb)
        .await;
    match res {
        Ok(msg) => {
            store.set_message_id(&token, msg.id).await;
            tracing::info!(outcome = ?SessionOutcome::ChallengeSent, "Dispatched join verification challenge");
            Ok(SessionOutcome::ChallengeSent)
        }
        Err(err) => {
            store.remove_by_token(&token).await;
            let out = match classify_request_error(&err) {
                BotErrorAction::UserBlockedBot => SessionOutcome::LeftPendingBlocked,
                BotErrorAction::DmWindowExpired => SessionOutcome::LeftPendingExpired,
                BotErrorAction::IdempotentNoOp | BotErrorAction::IdempotentSuccess => {
                    return Ok(SessionOutcome::IdempotentNoOp);
                }
                _ => return Err(err),
            };
            tracing::info!(outcome = ?out, "DM delivery failed; leaving request pending");
            Ok(out)
        }
    }
}
