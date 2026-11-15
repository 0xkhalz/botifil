use std::sync::Arc;
use teloxide::{
    RequestError,
    payloads::SendMessageSetters,
    requests::Requester,
    types::{ChatJoinRequest, InlineKeyboardButton, InlineKeyboardMarkup, WebAppInfo},
};

use crate::{
    config::Config,
    domain::{CallbackAction, Challenge, DomainError, PendingJoin, SessionOutcome, SessionToken},
    error::{BotErrorAction, classify_request_error},
    screening::{ScreeningVerdict, evaluate_applicant},
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

/// Builds the inline keyboard with a Telegram `WebApp` verification button.
///
/// # Errors
/// Returns [`DomainError::InvalidToken`] if `base_url` cannot host a query string.
pub fn build_webapp_keyboard(
    base_url: &url::Url,
    token: &SessionToken,
) -> Result<InlineKeyboardMarkup, DomainError> {
    if base_url.cannot_be_a_base() {
        return Err(DomainError::InvalidToken);
    }
    let mut url = base_url.clone();
    url.query_pairs_mut().append_pair("token", token.as_str());
    let btn = InlineKeyboardButton::web_app("🛡️ Open Verification", WebAppInfo { url });
    Ok(InlineKeyboardMarkup::new([[btn]]))
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
    let is_premium = match evaluate_applicant(req.bio.as_deref(), req.from.is_premium) {
        ScreeningVerdict::Decline { reason } => {
            return match bot
                .decline_chat_join_request(req.chat.id, req.from.id)
                .await
            {
                Ok(_) => {
                    tracing::info!(
                        outcome = ?SessionOutcome::DeclinedScreened,
                        ?reason,
                        "Declined join request via bio screening"
                    );
                    Ok(SessionOutcome::DeclinedScreened)
                }
                Err(err) => match classify_request_error(&err) {
                    BotErrorAction::IdempotentNoOp => Ok(SessionOutcome::IdempotentNoOp),
                    BotErrorAction::IdempotentSuccess => {
                        Ok(SessionOutcome::IdempotentAlreadyMember)
                    }
                    _ => Err(err),
                },
            };
        }
        ScreeningVerdict::Allow { is_premium } => is_premium,
    };
    let (token, challenge, now) = (
        SessionToken::generate(),
        Challenge::generate(),
        tokio::time::Instant::now(),
    );
    let (prompt, kb_res) = if let Some(base_url) = config.webapp_url() {
        (
            "Tap below to verify your join request.".to_owned(),
            build_webapp_keyboard(base_url, &token),
        )
    } else {
        (
            challenge.prompt_text(),
            build_challenge_keyboard(&token, challenge.choices),
        )
    };
    let Ok(kb) = kb_res else {
        tracing::warn!("Failed to build challenge keyboard; skipping request");
        return Ok(SessionOutcome::IdempotentNoOp);
    };
    let pending = PendingJoin {
        token: token.clone(),
        chat_id: req.chat.id,
        user_id: req.from.id,
        user_chat_id: req.user_chat_id,
        challenge,
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
        .send_message(req.user_chat_id, prompt)
        .reply_markup(kb)
        .await;
    match res {
        Ok(msg) => {
            store.set_message_id(&token, msg.id).await;
            let out = SessionOutcome::ChallengeSent;
            tracing::info!(outcome = ?out, is_premium, "Dispatched join verification challenge");
            Ok(out)
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
