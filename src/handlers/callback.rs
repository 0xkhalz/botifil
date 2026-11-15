use teloxide::{
    RequestError,
    payloads::AnswerCallbackQuerySetters,
    requests::Requester,
    types::{CallbackQuery, CallbackQueryId},
};

use crate::{
    domain::{CallbackAction, PendingJoin, SessionOutcome},
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

async fn finish<B: Requester<Err = RequestError>>(
    bot: &B,
    id: CallbackQueryId,
    s: PendingJoin,
    pass: bool,
) -> Result<SessionOutcome, RequestError> {
    match crate::webapp::resolve_join_request(bot, &s, pass).await {
        Ok(SessionOutcome::Approved) => {
            toast(bot, id, "Verification passed!").await;
            Ok(SessionOutcome::Approved)
        }
        Ok(SessionOutcome::DeclinedExhausted) => {
            toast(bot, id, "Verification failed.").await;
            Ok(SessionOutcome::DeclinedExhausted)
        }
        Ok(SessionOutcome::IdempotentNoOp) => {
            alert(
                bot,
                id,
                "Your join request is no longer pending (it was dismissed or already handled by an admin).",
            )
            .await;
            Ok(SessionOutcome::IdempotentNoOp)
        }
        Ok(SessionOutcome::IdempotentAlreadyMember) => {
            toast(bot, id, "You are already a member of this chat.").await;
            Ok(SessionOutcome::IdempotentAlreadyMember)
        }
        Ok(other) => Ok(other),
        Err(err) => {
            alert(
                bot,
                id,
                "Unable to process your join request right now. Please try again later.",
            )
            .await;
            Err(err)
        }
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
