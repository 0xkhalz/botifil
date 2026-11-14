pub mod callback;
pub mod join_request;

use std::sync::Arc;
use teloxide::{
    RequestError,
    dispatching::{UpdateFilterExt, UpdateHandler},
    dptree,
    requests::Requester,
    types::{CallbackQuery, ChatJoinRequest, ChatMemberUpdated, Update},
};

use crate::{config::Config, store::SessionStore};

/// Handles chat member updates by evicting stale pending sessions.
///
/// # Errors
/// Returns `Ok(())` unconditionally.
pub async fn handle_chat_member(
    upd: ChatMemberUpdated,
    store: SessionStore,
) -> Result<(), RequestError> {
    store
        .remove_by_peer(upd.chat.id, upd.new_chat_member.user.id)
        .await;
    Ok(())
}

/// Constructs the Telegram update dispatcher schema for botifil.
pub fn schema<B>() -> UpdateHandler<RequestError>
where
    B: Requester<Err = RequestError> + Clone + Send + Sync + 'static,
{
    dptree::entry()
        .branch(Update::filter_chat_join_request().endpoint(
            |b: B, r: ChatJoinRequest, c: Arc<Config>, s: SessionStore| async move {
                join_request::handle_join_request(b, r, c, s)
                    .await
                    .map(|_| ())
            },
        ))
        .branch(Update::filter_callback_query().endpoint(
            |b: B, q: CallbackQuery, s: SessionStore| async move {
                callback::handle_callback_query(b, q, s).await.map(|_| ())
            },
        ))
        .branch(Update::filter_chat_member().endpoint(handle_chat_member))
}

#[cfg(test)]
mod tests {
    use super::{join_request::build_challenge_keyboard, *};
    use crate::domain::{CallbackAction, MAX_CALLBACK_DATA_BYTES, SessionToken};
    use teloxide::{
        Bot,
        adaptors::{Throttle, throttle::Limits},
        dispatching::Dispatcher,
        requests::RequesterExt,
        types::InlineKeyboardButtonKind,
    };

    #[test]
    fn test_build_challenge_keyboard() -> Result<(), Box<dyn std::error::Error>> {
        let tok = SessionToken::generate();
        let items = build_challenge_keyboard(&tok, [2, 7, 9])?.inline_keyboard;
        for btn in items.into_iter().flatten() {
            let InlineKeyboardButtonKind::CallbackData(d) = btn.kind else {
                return Err("expected callback button".into());
            };
            assert!(d.len() <= MAX_CALLBACK_DATA_BYTES && CallbackAction::decode(&d)?.token == tok);
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_schema_type_check() -> Result<(), Box<dyn std::error::Error>> {
        let b = Bot::new("t").throttle(Limits::default());
        let cfg = Arc::new(Config::from_token(Some("t".into()))?);
        let _ = Dispatcher::builder(b, schema::<Throttle<Bot>>())
            .dependencies(dptree::deps![cfg, SessionStore::default()])
            .build();
        Ok(())
    }
}
