use std::{path::PathBuf, sync::Arc, time::Duration};

use botifil::handlers::{callback::handle_callback_query, join_request::handle_join_request};
use botifil::store::{DEFAULT_MAX_CAPACITY as CAP, SessionStore, reconciliation as rec};
use botifil::{config::Config, domain::SessionOutcome};
use teloxide::Bot;
use teloxide::types::{CallbackQuery, CallbackQueryId, Chat, ChatId, ChatJoinRequest, ChatKind};
use teloxide::types::{ChatPrivate, User, UserId};
use tokio::time::Instant;
use url::Url;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

type Res<T = ()> = Result<T, Box<dyn std::error::Error>>;
#[rustfmt::skip]
type StartRes = Res<(TempDb, MockServer, Bot, Arc<Config>, SessionStore, String, String)>;
#[rustfmt::skip]
const OK_M: &str = r#"{"ok":true,"result":{"message_id":42,"date":1,"chat":{"id":777,"type":"private"}}}"#;

struct TempDb(PathBuf);
#[rustfmt::skip]
impl TempDb {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("botifil-persist-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }
}
#[rustfmt::skip]
impl Drop for TempDb { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
#[rustfmt::skip]
const fn stats(evicted: usize, active: usize) -> rec::ReconcileStats { rec::ReconcileStats { evicted, active } }

#[rustfmt::skip]
fn user() -> User {
    User { id: UserId(123), is_bot: false, first_name: "T".into(), last_name: None, username: None, language_code: None, is_premium: false, added_to_attachment_menu: false }
}
#[rustfmt::skip]
#[allow(clippy::default_trait_access)]
fn join_req() -> ChatJoinRequest {
    let kind = ChatKind::Private(ChatPrivate { first_name: None, last_name: None, username: None });
    ChatJoinRequest { chat: Chat { id: ChatId(-100), kind }, from: user(), user_chat_id: ChatId(777), date: Default::default(), bio: None, invite_link: None }
}
#[rustfmt::skip]
fn cb(data: String) -> CallbackQuery {
    CallbackQuery { id: CallbackQueryId("1".into()), from: user(), message: None, inline_message_id: None, chat_instance: "i".into(), data: Some(data), game_short_name: None }
}

#[rustfmt::skip]
async fn start(name: &str) -> StartRes {
    let (tmp, server, ok) = (TempDb::new(name), MockServer::start().await, r#"{"ok":true,"result":true}"#);
    for (route, body) in [("SendMessage", OK_M), ("EditMessageText", OK_M), ("AnswerCallbackQuery", ok), ("ApproveChatJoinRequest", ok), ("DeclineChatJoinRequest", ok)] {
        Mock::given(path(format!("/bottest_token/{route}"))).respond_with(ResponseTemplate::new(200).set_body_string(body)).mount(&server).await;
    }
    let bot = Bot::new("test_token").set_api_url(Url::parse(&server.uri())?);
    let db_path = tmp.0.join("botifil.redb");
    let db_str = db_path.to_str().ok_or("non-utf8 temp path")?;
    let cfg = Arc::new(Config::from_env_vars(Some("test_token".into()), None, None, None, Some(db_str))?);
    let store = SessionStore::open_redb(cfg.db_path(), CAP)?;
    let out = handle_join_request(bot.clone(), join_req(), Arc::clone(&cfg), store.clone()).await?;
    assert_eq!(out, SessionOutcome::ChallengeSent);
    let (correct, wrong) = choices(&server).await?;
    Ok((tmp, server, bot, cfg, store, correct, wrong))
}

#[rustfmt::skip]
async fn paths(server: &MockServer) -> Res<Vec<String>> {
    Ok(server.received_requests().await.ok_or("no requests")?.into_iter().map(|r| r.url.path().to_string()).collect())
}
#[rustfmt::skip]
fn has(routes: &[String], target: &str) -> bool { routes.iter().any(|r| r.ends_with(target)) }
#[rustfmt::skip]
fn idx(routes: &[String], target: &str) -> Res<usize> {
    routes.iter().position(|r| r.ends_with(target)).ok_or_else(|| "missing route".into())
}

#[rustfmt::skip]
async fn choices(server: &MockServer) -> Res<(String, String)> {
    let reqs = server.received_requests().await.ok_or("no requests")?;
    let body = String::from_utf8_lossy(&reqs.first().ok_or("missing request")?.body);
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
async fn active_challenge_survives_reopen_and_approves() -> Res {
    let (_tmp, srv, bot, cfg, store, ok, _) = start("approve").await?;
    drop(store);
    let st = SessionStore::open_redb(cfg.db_path(), CAP)?;
    assert_eq!(
        rec::reconcile_on_startup(&st, Instant::now()).await,
        stats(0, 1)
    );
    let out = handle_callback_query(bot, cb(ok), st.clone()).await?;
    assert_eq!(out, SessionOutcome::Approved);
    assert!(st.is_empty().await);
    let r = paths(&srv).await?;
    assert!(idx(&r, "/EditMessageText")? < idx(&r, "/ApproveChatJoinRequest")?);
    Ok(())
}

#[tokio::test]
async fn wrong_attempts_persist_across_reopen_and_decline() -> Res {
    let (_tmp, srv, bot, cfg, store, _, wrong) = start("decline").await?;
    let retry = handle_callback_query(bot.clone(), cb(wrong.clone()), store.clone()).await?;
    assert_eq!(retry, SessionOutcome::RetryAllowed { remaining: 1 });
    assert!(!has(&paths(&srv).await?, "/DeclineChatJoinRequest"));
    drop(store);
    let st = SessionStore::open_redb(cfg.db_path(), CAP)?;
    assert_eq!(
        rec::reconcile_on_startup(&st, Instant::now()).await,
        stats(0, 1)
    );
    let out = handle_callback_query(bot, cb(wrong), st.clone()).await?;
    assert_eq!(out, SessionOutcome::DeclinedExhausted);
    assert!(st.is_empty().await);
    let r = paths(&srv).await?;
    assert!(has(&r, "/EditMessageText") && has(&r, "/DeclineChatJoinRequest"));
    Ok(())
}

#[tokio::test]
async fn expired_session_evicted_on_startup_leaves_pending() -> Res {
    let (_tmp, srv, bot, cfg, store, ok, _) = start("expired").await?;
    drop(store);
    let st = SessionStore::open_redb(cfg.db_path(), CAP)?;
    let later = Instant::now() + Duration::from_secs(600);
    assert_eq!(rec::reconcile_on_startup(&st, later).await, stats(1, 0));
    assert!(st.is_empty().await);
    let (out, r) = (
        handle_callback_query(bot, cb(ok), st).await?,
        paths(&srv).await?,
    );
    assert_eq!(out, SessionOutcome::LeftPendingExpired);
    assert!(!has(&r, "/DeclineChatJoinRequest") && !has(&r, "/ApproveChatJoinRequest"));
    Ok(())
}
