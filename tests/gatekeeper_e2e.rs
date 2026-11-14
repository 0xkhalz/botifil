use std::sync::Arc;

use botifil::{
    config::Config,
    domain::SessionOutcome,
    handlers::{callback::handle_callback_query, join_request::handle_join_request},
    store::SessionStore,
};
use teloxide::{
    Bot,
    types::{
        CallbackQuery, CallbackQueryId, Chat, ChatId, ChatJoinRequest, ChatKind, ChatPrivate, User,
        UserId,
    },
};
use url::Url;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

type Res<T = ()> = Result<T, Box<dyn std::error::Error>>;
type StartRes = Res<(MockServer, Bot, SessionStore, SessionOutcome)>;
const OK_M: &str =
    r#"{"ok":true,"result":{"message_id":42,"date":1,"chat":{"id":777,"type":"private"}}}"#;

#[rustfmt::skip]
fn user() -> User {
    User { id: UserId(123), is_bot: false, first_name: "T".into(), last_name: None, username: None, language_code: None, is_premium: false, added_to_attachment_menu: false }
}

#[rustfmt::skip]
#[allow(clippy::default_trait_access)]
fn req() -> ChatJoinRequest {
    let kind = ChatKind::Private(ChatPrivate { first_name: None, last_name: None, username: None });
    ChatJoinRequest { chat: Chat { id: ChatId(-100), kind }, from: user(), user_chat_id: ChatId(777), date: Default::default(), bio: None, invite_link: None }
}

#[rustfmt::skip]
fn cb(data: String) -> CallbackQuery {
    CallbackQuery { id: CallbackQueryId("1".into()), from: user(), message: None, inline_message_id: None, chat_instance: "i".into(), data: Some(data), game_short_name: None }
}

#[rustfmt::skip]
async fn start(ovr: Option<(&str, u16, &str)>) -> StartRes {
    let (server, ok) = (MockServer::start().await, r#"{"ok":true,"result":true}"#);
    for (name, def) in [("SendMessage", OK_M), ("EditMessageText", OK_M), ("AnswerCallbackQuery", ok), ("ApproveChatJoinRequest", ok), ("DeclineChatJoinRequest", ok)] {
        let (code, body) = match ovr { Some((target, status, custom)) if target == name => (status, custom), _ => (200, def) };
        Mock::given(path(format!("/bottest_token/{name}"))).respond_with(ResponseTemplate::new(code).set_body_string(body)).mount(&server).await;
    }
    let (bot, store) = (Bot::new("test_token").set_api_url(Url::parse(&server.uri())?), SessionStore::default());
    let outcome = handle_join_request(bot.clone(), req(), Arc::new(Config::from_token(Some("test_token".into()))?), store.clone()).await?;
    Ok((server, bot, store, outcome))
}

#[rustfmt::skip]
async fn paths(server: &MockServer) -> Res<Vec<String>> {
    Ok(server.received_requests().await.ok_or("no requests")?.into_iter().map(|r| r.url.path().to_string()).collect())
}
fn has(routes: &[String], target: &str) -> bool {
    routes.iter().any(|r| r.ends_with(target))
}
fn idx(routes: &[String], target: &str) -> Res<usize> {
    routes
        .iter()
        .position(|r| r.ends_with(target))
        .ok_or_else(|| "missing route".into())
}

#[rustfmt::skip]
async fn choices(server: &MockServer) -> Res<(String, String)> {
    let reqs = server.received_requests().await.ok_or("no requests")?;
    let body = String::from_utf8_lossy(&reqs[0].body);
    let (prefix, query) = (body.find("what is ").ok_or("prefix")? + 8, body.find('?').ok_or("query")?);
    let mut parts = body[prefix..query].split(" + ");
    let sum: u8 = parts.next().ok_or("left")?.parse::<u8>()? + parts.next().ok_or("right")?.parse::<u8>()?;
    let (mut correct, mut wrong, mut cursor) = (String::new(), String::new(), body.as_ref());
    while let Some(index) = cursor.find("\"callback_data\":\"") {
        let tail = &cursor[index + 17..];
        let val = &tail[..tail.find('"').ok_or("quote")?];
        *(if val.ends_with(&format!(":{sum}")) { &mut correct } else { &mut wrong }) = val.into();
        cursor = tail;
    }
    Ok((correct, wrong))
}

#[tokio::test]
async fn e2e_join_request_correct_answer_approves_and_edits_dm() -> Res {
    let (server, bot, store, outcome) = start(None).await?;
    assert_eq!(outcome, SessionOutcome::ChallengeSent);
    let reqs = server.received_requests().await.ok_or("no requests")?;
    assert!(String::from_utf8_lossy(&reqs[0].body).contains("\"chat_id\":777"));

    let (correct, _) = choices(&server).await?;
    assert_eq!(
        handle_callback_query(bot, cb(correct), store).await?,
        SessionOutcome::Approved
    );

    let routes = paths(&server).await?;
    assert!(idx(&routes, "/EditMessageText")? < idx(&routes, "/ApproveChatJoinRequest")?);
    Ok(())
}

#[tokio::test]
async fn e2e_wrong_answers_retry_then_decline_on_max_attempts() -> Res {
    let (server, bot, store, outcome) = start(None).await?;
    assert_eq!(outcome, SessionOutcome::ChallengeSent);
    let (_, wrong) = choices(&server).await?;

    let out1 = handle_callback_query(bot.clone(), cb(wrong.clone()), store.clone()).await?;
    assert_eq!(out1, SessionOutcome::RetryAllowed { remaining: 1 });
    assert!(!has(&paths(&server).await?, "/DeclineChatJoinRequest"));

    let out2 = handle_callback_query(bot, cb(wrong), store).await?;
    assert_eq!(out2, SessionOutcome::DeclinedExhausted);
    let routes = paths(&server).await?;
    assert!(has(&routes, "/EditMessageText") && has(&routes, "/DeclineChatJoinRequest"));
    Ok(())
}

#[tokio::test]
async fn e2e_blocked_dm_403_leaves_request_pending_without_decline() -> Res {
    let err =
        r#"{"ok":false,"error_code":403,"description":"Forbidden: bot was blocked by the user"}"#;
    let (server, _bot, store, outcome) = start(Some(("SendMessage", 403, err))).await?;
    assert_eq!(outcome, SessionOutcome::LeftPendingBlocked);
    assert!(store.is_empty().await);
    assert!(!has(&paths(&server).await?, "/DeclineChatJoinRequest"));
    Ok(())
}

#[tokio::test]
async fn e2e_approve_hide_requester_missing_is_idempotent() -> Res {
    let err =
        r#"{"ok":false,"error_code":400,"description":"Bad Request: HIDE_REQUESTER_MISSING"}"#;
    let (server, bot, store, _) = start(Some(("ApproveChatJoinRequest", 400, err))).await?;
    let (correct, _) = choices(&server).await?;
    assert_eq!(
        handle_callback_query(bot, cb(correct), store).await?,
        SessionOutcome::IdempotentNoOp
    );
    Ok(())
}
