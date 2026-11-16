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

/// Builds the inline keyboard with a Telegram Mini App verification button.
pub fn build_webapp_keyboard(base_url: &url::Url, token: &SessionToken) -> InlineKeyboardMarkup {
    let mut url = base_url.clone();
    url.query_pairs_mut().append_pair("token", token.as_str());
    InlineKeyboardMarkup::new([[InlineKeyboardButton::web_app(
        "🛡️ Open Verification",
        WebAppInfo { url },
    )]])
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
    let (kb, dm_text) = if let Some(base_url) = config.webapp_url() {
        (
            build_webapp_keyboard(base_url, &token),
            "Tap the button below to verify and join:".to_owned(),
        )
    } else {
        let Ok(kb) = build_challenge_keyboard(&token, challenge.choices) else {
            tracing::warn!("Failed to build challenge keyboard; skipping request");
            return Ok(SessionOutcome::IdempotentNoOp);
        };
        (kb, challenge.prompt_text())
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
    if let Err(err) = store.upsert(pending, now).await {
        match err {
            StoreError::CapacityExceeded => {
                tracing::warn!("Session store capacity exceeded; dropping join request");
            }
            StoreError::Backend(reason) => {
                tracing::warn!(
                    error = %reason,
                    "Session store write failed; dropping join request"
                );
            }
        }
        return Ok(SessionOutcome::IdempotentNoOp);
    }
    let res = bot
        .send_message(req.user_chat_id, dm_text)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;
    use teloxide::{
        Bot,
        types::{
            Chat, ChatId, ChatKind, ChatPublic, InlineKeyboardButtonKind, PublicChatKind, User,
            UserId,
        },
    };
    use url::Url;
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    type TestRes<T> = Result<T, Box<dyn std::error::Error>>;

    #[allow(clippy::default_trait_access)]
    fn dummy_join_request() -> ChatJoinRequest {
        ChatJoinRequest {
            chat: Chat {
                id: ChatId(-100_123_456),
                kind: ChatKind::Public(ChatPublic {
                    title: Some("Test Group".into()),
                    kind: PublicChatKind::Group,
                }),
            },
            from: User {
                id: UserId(42),
                is_bot: false,
                first_name: "Alice".into(),
                last_name: None,
                username: None,
                language_code: None,
                is_premium: false,
                added_to_attachment_menu: false,
            },
            user_chat_id: ChatId(42),
            date: Default::default(),
            bio: None,
            invite_link: None,
        }
    }

    #[test]
    fn webapp_keyboard_appends_token_and_preserves_subpath_and_query() -> TestRes<()> {
        let tok = SessionToken::new("tokabc123")?;
        let plain = Url::parse("https://example.com")?;
        let kb = build_webapp_keyboard(&plain, &tok);
        assert_eq!(kb.inline_keyboard.len(), 1);
        assert_eq!(kb.inline_keyboard[0].len(), 1);
        let btn = &kb.inline_keyboard[0][0];
        assert_eq!(btn.text, "🛡️ Open Verification");
        let InlineKeyboardButtonKind::WebApp(info) = &btn.kind else {
            return Err("expected WebApp button".into());
        };
        assert_eq!(info.url.as_str(), "https://example.com/?token=tokabc123");

        let complex = Url::parse("https://example.com/verify/app?chat=test&lang=en")?;
        let kb2 = build_webapp_keyboard(&complex, &tok);
        let InlineKeyboardButtonKind::WebApp(info2) = &kb2.inline_keyboard[0][0].kind else {
            return Err("expected WebApp button".into());
        };
        assert_eq!(info2.url.path(), "/verify/app");
        let pairs: Vec<_> = info2.url.query_pairs().into_owned().collect();
        assert_eq!(
            pairs,
            vec![
                ("chat".to_owned(), "test".to_owned()),
                ("lang".to_owned(), "en".to_owned()),
                ("token".to_owned(), "tokabc123".to_owned()),
            ]
        );
        Ok(())
    }

    async fn mock_send_message() -> TestRes<(MockServer, Bot)> {
        let server = MockServer::start().await;
        let ok_msg = r#"{"ok":true,"result":{"message_id":77,"date":0,"chat":{"id":42,"type":"private"},"text":"ok"}}"#;
        Mock::given(path("/bottest_token/SendMessage"))
            .respond_with(ResponseTemplate::new(200).set_body_string(ok_msg))
            .mount(&server)
            .await;
        let bot = Bot::new("test_token").set_api_url(Url::parse(&server.uri())?);
        Ok((server, bot))
    }

    #[tokio::test]
    async fn handle_join_request_sends_webapp_button_when_configured() -> TestRes<()> {
        let (server, bot) = mock_send_message().await?;
        let cfg = Arc::new(Config::from_all_env_vars(
            Some("test_token".into()),
            None,
            None,
            None,
            None,
            Some("https://example.com/app?v=1"),
            None,
        )?);
        let store = SessionStore::from_storage(Arc::new(MemoryStore::new(8)));
        let out = handle_join_request(bot, dummy_join_request(), cfg, store).await?;
        assert_eq!(out, SessionOutcome::ChallengeSent);

        let reqs = server.received_requests().await.ok_or("no requests")?;
        assert_eq!(reqs.len(), 1);
        let body = std::str::from_utf8(&reqs[0].body)?;
        assert!(body.contains("Tap the button below to verify and join:"));
        assert!(body.contains("web_app"));
        assert!(body.contains("https://example.com/app?v=1&token="));
        Ok(())
    }

    #[tokio::test]
    async fn handle_join_request_falls_back_to_inline_keyboard_when_webapp_url_none() -> TestRes<()>
    {
        let (server, bot) = mock_send_message().await?;
        let cfg = Arc::new(Config::from_all_env_vars(
            Some("test_token".into()),
            None,
            None,
            None,
            None,
            None,
            None,
        )?);
        let store = SessionStore::from_storage(Arc::new(MemoryStore::new(8)));
        let out = handle_join_request(bot, dummy_join_request(), cfg, store).await?;
        assert_eq!(out, SessionOutcome::ChallengeSent);

        let reqs = server.received_requests().await.ok_or("no requests")?;
        assert_eq!(reqs.len(), 1);
        let body = std::str::from_utf8(&reqs[0].body)?;
        assert!(body.contains("callback_data"));
        assert!(body.contains("Verify you are human: what is "));
        assert!(!body.contains("web_app"));
        Ok(())
    }
}
