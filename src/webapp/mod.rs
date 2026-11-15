use teloxide::{RequestError, requests::Requester};

use crate::{
    domain::{PendingJoin, SessionOutcome},
    error::{BotErrorAction, classify_request_error},
};

pub mod auth;

/// Embedded Rose-Pine Telegram Mini App verification HTML.
pub const INDEX_HTML: &str = include_str!("ui.html");

async fn edit_dm<B: Requester>(bot: &B, session: &PendingJoin, text: impl Into<String>) {
    if let Some(m) = session.message_id {
        let _ = bot.edit_message_text(session.user_chat_id, m, text).await;
    }
}

/// Resolves a Telegram chat join request and updates the verification DM accordingly.
///
/// Enforces resolve-before-feedback ordering by calling `approve_chat_join_request`
/// or `decline_chat_join_request` first, handling idempotent Telegram API errors via
/// `classify_request_error`, and performing best-effort `edit_message_text` DM updates.
///
/// # Errors
/// Returns [`RequestError`] if an unhandled Telegram API error occurs during resolution.
pub async fn resolve_join_request<B: Requester<Err = RequestError>>(
    bot: &B,
    session: &PendingJoin,
    pass: bool,
) -> Result<SessionOutcome, RequestError> {
    let res = if pass {
        bot.approve_chat_join_request(session.chat_id, session.user_id)
            .await
    } else {
        bot.decline_chat_join_request(session.chat_id, session.user_id)
            .await
    };
    match res {
        Ok(_) => {
            let (dm_text, out) = if pass {
                (
                    "Verification passed. Your join request has been approved.",
                    SessionOutcome::Approved,
                )
            } else {
                (
                    "Verification failed. Your join request has been declined.",
                    SessionOutcome::DeclinedExhausted,
                )
            };
            edit_dm(bot, session, dm_text).await;
            Ok(out)
        }
        Err(err) => match classify_request_error(&err) {
            BotErrorAction::IdempotentNoOp => {
                edit_dm(
                    bot,
                    session,
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
                edit_dm(
                    bot,
                    session,
                    "Your join request was already approved; you are already a member of this chat.",
                )
                .await;
                tracing::info!(
                    outcome = ?SessionOutcome::IdempotentAlreadyMember,
                    "User already a chat member during resolution"
                );
                Ok(SessionOutcome::IdempotentAlreadyMember)
            }
            _ => Err(err),
        },
    }
}
