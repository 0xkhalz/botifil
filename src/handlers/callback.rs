use teloxide::{
    RequestError,
    payloads::AnswerCallbackQuerySetters,
    requests::Requester,
    types::{CallbackQuery, CallbackQueryId},
};

use crate::{
    domain::{CallbackAction, PendingJoin, SessionOutcome},
    error::{BotErrorAction, classify_request_error},
    store::{AttemptOutcome, SessionStore},
};

async fn alert<B: Requester>(bot: &B, id: CallbackQueryId, text: impl Into<String>) {
    let _ = bot
        .answer_callback_query(id)
        .text(text)
        .show_alert(true)
        .await;
}

async fn toast<B: Requester>(bot: &B, id: CallbackQueryId, text: impl Into<String>) {
    let _ = bot.answer_callback_query(id).text(text).await;
}

async fn edit_dm<B: Requester>(bot: &B, s: &PendingJoin, text: impl Into<String>) {
    if let Some(m) = s.message_id {
        let _ = bot.edit_message_text(s.user_chat_id, m, text).await;
    }
}

async fn finish<B: Requester<Err = RequestError>>(
    bot: &B,
    id: CallbackQueryId,
    s: PendingJoin,
    pass: bool,
) -> Result<SessionOutcome, RequestError> {
    let res = if pass {
        bot.approve_chat_join_request(s.chat_id, s.user_id).await
    } else {
        bot.decline_chat_join_request(s.chat_id, s.user_id).await
    };
    match res {
        Ok(_) => {
            let (cb_text, dm_text, out) = if pass {
                (
                    "Verification passed!",
                    "Verification passed. Your join request has been approved.",
                    SessionOutcome::Approved,
                )
            } else {
                (
                    "Verification failed.",
                    "Verification failed. Your join request has been declined.",
                    SessionOutcome::DeclinedExhausted,
                )
            };
            toast(bot, id, cb_text).await;
            edit_dm(bot, &s, dm_text).await;
            Ok(out)
        }
        Err(err) => match classify_request_error(&err) {
            BotErrorAction::IdempotentNoOp => {
                alert(
                    bot,
                    id,
                    "Your join request is no longer pending (it was dismissed or already handled by an admin).",
                )
                .await;
                edit_dm(
                    bot,
                    &s,
                    "This join request is no longer pending (it was already dismissed or handled by an admin).",
                )
                .await;
                tracing::info!(
                    outcome = ?SessionOutcome::IdempotentNoOp,
                    "Join request no longer pending during resolution"
                );
                Ok(SessionOutcome::IdempotentNoOp)
            }
            BotErrorAction::IdempotentSuccess => {
                toast(bot, id, "You are already a member of this chat.").await;
                edit_dm(
                    bot,
                    &s,
                    "Your join request was already approved; you are already a member of this chat.",
                )
                .await;
                tracing::info!(
                    outcome = ?SessionOutcome::IdempotentAlreadyMember,
                    "User already a chat member during resolution"
                );
                Ok(SessionOutcome::IdempotentAlreadyMember)
            }
            _ => {
                alert(
                    bot,
                    id,
                    "Unable to process your join request right now. Please try again later.",
                )
                .await;
                Err(err)
            }
        },
    }
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
