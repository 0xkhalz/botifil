use teloxide::{
    RequestError,
    payloads::AnswerCallbackQuerySetters,
    requests::Requester,
    types::{CallbackQuery, CallbackQueryId, True},
};

use crate::{
    domain::{CallbackAction, PendingJoin, SessionOutcome},
    error::{BotErrorAction, classify_request_error},
    store::{AttemptOutcome, SessionStore},
};

fn resolve_with_idempotency(
    res: Result<True, RequestError>,
    default_outcome: SessionOutcome,
) -> Result<SessionOutcome, RequestError> {
    match res {
        Ok(_) => Ok(default_outcome),
        Err(err) => match classify_request_error(&err) {
            BotErrorAction::IdempotentNoOp => Ok(SessionOutcome::IdempotentNoOp),
            BotErrorAction::IdempotentSuccess => Ok(SessionOutcome::IdempotentAlreadyMember),
            _ => Err(err),
        },
    }
}

async fn alert<B: Requester>(bot: &B, id: CallbackQueryId, text: impl Into<String>) {
    let _ = bot
        .answer_callback_query(id)
        .text(text)
        .show_alert(true)
        .await;
}

async fn finish<B: Requester<Err = RequestError>>(
    bot: &B,
    id: CallbackQueryId,
    s: PendingJoin,
    pass: bool,
) -> Result<SessionOutcome, RequestError> {
    let (toast, msg, out) = if pass {
        let t = "Verification passed. Your join request has been approved.";
        ("Verification passed!", t, SessionOutcome::Approved)
    } else {
        let t = "Verification failed. Your join request has been declined.";
        ("Verification failed.", t, SessionOutcome::DeclinedExhausted)
    };
    let _ = bot.answer_callback_query(id).text(toast).await;
    if let Some(m) = s.message_id {
        let _ = bot.edit_message_text(s.user_chat_id, m, msg).await;
    }
    let res = if pass {
        bot.approve_chat_join_request(s.chat_id, s.user_id).await
    } else {
        bot.decline_chat_join_request(s.chat_id, s.user_id).await
    };
    resolve_with_idempotency(res, out)
}

/// Handles an incoming callback query update from an inline verification challenge button.
///
/// # Errors
/// Returns [`RequestError`] if an unhandled Telegram API error occurs during verification resolution.
pub async fn handle_callback_query<B>(
    bot: B,
    q: CallbackQuery,
    store: SessionStore,
) -> Result<SessionOutcome, RequestError>
where
    B: Requester<Err = RequestError>,
{
    let Some(action) = q
        .data
        .as_deref()
        .and_then(|d| CallbackAction::decode(d).ok())
    else {
        alert(&bot, q.id, "Invalid verification payload.").await;
        return Ok(SessionOutcome::IdempotentNoOp);
    };

    let now = tokio::time::Instant::now();
    match store
        .verify_attempt(&action.token, q.from.id, action.answer, now)
        .await
    {
        AttemptOutcome::WrongUser => {
            let msg = "This verification challenge belongs to another user.";
            alert(&bot, q.id, msg).await;
            Ok(SessionOutcome::IdempotentNoOp)
        }
        AttemptOutcome::NotFoundOrExpired => {
            let msg = "Verification expired; request remains pending admin review.";
            alert(&bot, q.id, msg).await;
            Ok(SessionOutcome::LeftPendingExpired)
        }
        AttemptOutcome::Retry { remaining: rem } => {
            let msg = format!("Incorrect answer. {rem} attempt(s) remaining.");
            alert(&bot, q.id, msg).await;
            Ok(SessionOutcome::RetryAllowed { remaining: rem })
        }
        AttemptOutcome::Approved(s) => finish(&bot, q.id, s, true).await,
        AttemptOutcome::DeclinedExhausted(s) => finish(&bot, q.id, s, false).await,
    }
}
