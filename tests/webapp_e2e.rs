use std::{
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use botifil::{
    config::Config,
    domain::{SessionOutcome, SessionToken},
    handlers::join_request::handle_join_request,
    store::SessionStore,
    webapp::{auth::sign_init_data_for_test, server::serve_webapp},
};
use teloxide::{
    Bot,
    types::{Chat, ChatId, ChatJoinRequest, ChatKind, ChatPrivate, User, UserId},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinHandle,
};
use url::{Url, form_urlencoded};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

type Res<T = ()> = Result<T, Box<dyn std::error::Error>>;
const OK_M: &str =
    r#"{"ok":true,"result":{"message_id":42,"date":1,"chat":{"id":777,"type":"private"}}}"#;
const WEBAPP_URL: &str = "https://bot.example/verify";

struct Harness {
    mock: MockServer,
    store: SessionStore,
    addr: SocketAddr,
    tx: watch::Sender<bool>,
    task: JoinHandle<()>,
    token: String,
}
impl Harness {
    async fn stop(self) {
        let _ = self.tx.send(true);
        let _ = self.task.await;
    }
}

#[rustfmt::skip]
#[allow(clippy::default_trait_access)]
fn join_req() -> ChatJoinRequest {
    let kind = ChatKind::Private(ChatPrivate { first_name: None, last_name: None, username: None });
    let from = User { id: UserId(123), is_bot: false, first_name: "T".into(), last_name: None, username: None, language_code: None, is_premium: false, added_to_attachment_menu: false };
    ChatJoinRequest { chat: Chat { id: ChatId(-100), kind }, from, user_chat_id: ChatId(777), date: Default::default(), bio: None, invite_link: None }
}

#[rustfmt::skip]
async fn setup() -> Res<Harness> {
    let (mock, ok) = (MockServer::start().await, r#"{"ok":true,"result":true}"#);
    for (name, def) in [("SendMessage", OK_M), ("EditMessageText", OK_M), ("ApproveChatJoinRequest", ok), ("DeclineChatJoinRequest", ok)] {
        Mock::given(path(format!("/bottest_token/{name}"))).respond_with(ResponseTemplate::new(200).set_body_string(def)).mount(&mock).await;
    }
    let bot = Bot::new("test_token").set_api_url(Url::parse(&mock.uri())?);
    let cfg = Arc::new(Config::from_all_env_vars(Some("test_token".into()), None, None, None, Some(WEBAPP_URL), None)?);
    let (store, listener) = (SessionStore::default(), TcpListener::bind("127.0.0.1:0").await?);
    let (addr, (tx, rx)) = (listener.local_addr()?, watch::channel(false));
    let task = tokio::spawn(serve_webapp(listener, bot.clone(), cfg.clone(), store.clone(), rx));
    assert_eq!(handle_join_request(bot, join_req(), cfg, store.clone()).await?, SessionOutcome::ChallengeSent);
    let reqs = mock.received_requests().await.ok_or("no requests")?;
    let body = String::from_utf8_lossy(&reqs[0].body);
    let prefix = format!("{WEBAPP_URL}?token=");
    let start = body.find(&prefix).ok_or("missing webapp url")? + prefix.len();
    let end = body[start..].find('"').ok_or("missing quote")?;
    Ok(Harness { mock, store, addr, tx, task, token: body[start..start + end].to_owned() })
}

async fn http_raw(addr: SocketAddr, head: &str, body: Option<&str>) -> Res<(u16, String, String)> {
    let mut stream = TcpStream::connect(addr).await?;
    stream.write_all(head.as_bytes()).await?;
    if let Some(payload) = body {
        stream.flush().await?;
        stream.write_all(payload.as_bytes()).await?;
    }
    let (mut raw, mut buf) = (Vec::new(), [0_u8; 4096]);
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset && !raw.is_empty() => break,
            Err(e) => return Err(e.into()),
        }
    }
    let text = String::from_utf8(raw)?;
    let (hdrs, resp_body) = text.split_once("\r\n\r\n").ok_or("bad http response")?;
    let line = hdrs.lines().next().ok_or("missing status line")?;
    let code: u16 = line
        .split_ascii_whitespace()
        .nth(1)
        .ok_or("missing code")?
        .parse()?;
    Ok((code, hdrs.to_ascii_lowercase(), resp_body.to_owned()))
}

async fn get(addr: SocketAddr, target: &str) -> Res<(u16, String, String)> {
    http_raw(
        addr,
        &format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n"),
        None,
    )
    .await
}

#[rustfmt::skip]
async fn post_verify(addr: SocketAddr, tok: &str, ans: u8, init: &str, split: bool) -> Res<(u16, String)> {
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("token", tok).append_pair("answer", &ans.to_string()).append_pair("init_data", init).finish();
    let head = format!("POST /api/verify HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n", body.len());
    let res = if split { http_raw(addr, &head, Some(&body)).await? } else { http_raw(addr, &format!("{head}{body}"), None).await? };
    Ok((res.0, res.2))
}

fn sign(uid: u64, bot_token: &str) -> Res<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs()
        .to_string();
    let user = format!(r#"{{"id":{uid},"first_name":"T"}}"#);
    Ok(sign_init_data_for_test(
        &[("auth_date", &now), ("query_id", "AAHdF"), ("user", &user)],
        bot_token,
    ))
}

fn json_u8(body: &str, key: &str) -> Res<u8> {
    let pat = format!("\"{key}\":");
    let rest = &body[body.find(&pat).ok_or("missing key")? + pat.len()..];
    Ok(rest[..rest.bytes().take_while(u8::is_ascii_digit).count()].parse()?)
}

async fn paths(mock: &MockServer) -> Res<Vec<String>> {
    let reqs = mock.received_requests().await.ok_or("no requests")?;
    Ok(reqs.into_iter().map(|r| r.url.path().to_string()).collect())
}

fn idx(routes: &[String], target: &str) -> Res<usize> {
    routes
        .iter()
        .position(|r| r.ends_with(target))
        .ok_or_else(|| "missing route".into())
}

#[tokio::test]
async fn webapp_dm_sends_webapp_button_with_token() -> Res {
    let h = setup().await?;
    assert!(SessionToken::new(h.token.clone()).is_ok());
    let reqs = h.mock.received_requests().await.ok_or("no requests")?;
    let body = String::from_utf8_lossy(&reqs[0].body);
    assert!(body.contains("\"chat_id\":777") && body.contains("Open Verification"));
    assert!(body.contains("Tap below to verify your join request."));
    assert!(body.contains(&format!(
        "\"web_app\":{{\"url\":\"{WEBAPP_URL}?token={}\"}}",
        h.token
    )));
    h.stop().await;
    Ok(())
}

#[tokio::test]
async fn webapp_serves_html_and_non_mutating_challenge() -> Res {
    let h = setup().await?;
    let (code, hdrs, html) = get(h.addr, &format!("/?token={}", h.token)).await?;
    assert_eq!(code, 200);
    assert!(hdrs.contains("content-type: text/html; charset=utf-8"));
    assert!(
        hdrs.contains("cache-control: no-store")
            && hdrs.contains("x-content-type-options: nosniff")
    );
    assert!(hdrs.contains("content-security-policy: default-src 'none'; script-src 'unsafe-inline' https://telegram.org; style-src 'unsafe-inline'; connect-src 'self'"));
    assert!(
        html.contains("Human Verification")
            && html.contains("#191724")
            && html.contains("Telegram?.WebApp")
    );

    let ch_path = format!("/api/challenge?token={}", h.token);
    let (c1, _, b1) = get(h.addr, &ch_path).await?;
    let (c2, _, b2) = get(h.addr, &ch_path).await?;
    assert_eq!((c1, c2), (200, 200));
    assert!(
        b1.contains("\"attempts_left\":2")
            && b1.contains("\"choices\":[")
            && b1.contains("\"remaining_secs\":")
    );
    assert_eq!(
        (json_u8(&b1, "left")?, json_u8(&b1, "right")?),
        (json_u8(&b2, "left")?, json_u8(&b2, "right")?)
    );
    let (bad, miss, other) = (
        get(h.addr, "/api/challenge?token=bad!").await?.0,
        get(h.addr, "/api/challenge?token=missingtok1").await?.0,
        get(h.addr, "/other").await?.0,
    );
    assert_eq!((bad, miss, other), (400, 404, 404));
    let big = format!("GET / HTTP/1.1\r\nX-Pad: {}\r\n\r\n", "a".repeat(17_000));
    assert_eq!(http_raw(h.addr, &big, None).await?.0, 413);
    h.stop().await;
    Ok(())
}

#[tokio::test]
async fn webapp_verify_correct_edits_dm_before_approve() -> Res {
    let h = setup().await?;
    let ch_path = format!("/api/challenge?token={}", h.token);
    let (_, _, ch) = get(h.addr, &ch_path).await?;
    let ans = json_u8(&ch, "left")? + json_u8(&ch, "right")?;
    let res = post_verify(h.addr, &h.token, ans, &sign(123, "test_token")?, false).await?;
    assert_eq!(res, (200, r#"{"status":"approved"}"#.into()));
    let routes = paths(&h.mock).await?;
    assert!(idx(&routes, "/EditMessageText")? < idx(&routes, "/ApproveChatJoinRequest")?);
    assert!(h.store.is_empty().await && get(h.addr, &ch_path).await?.0 == 404);
    h.stop().await;
    Ok(())
}

#[tokio::test]
async fn webapp_verify_wrong_user_and_retry_then_decline() -> Res {
    let h = setup().await?;
    let ch_path = format!("/api/challenge?token={}", h.token);
    let (_, _, ch) = get(h.addr, &ch_path).await?;
    let correct = json_u8(&ch, "left")? + json_u8(&ch, "right")?;
    let wrong = correct + 1;

    let wu = post_verify(h.addr, &h.token, correct, &sign(999, "test_token")?, true).await?;
    assert_eq!(
        wu,
        (403, r#"{"status":"error","error":"wrong_user"}"#.into())
    );
    let bad = post_verify(h.addr, &h.token, correct, &sign(123, "bad_token")?, false).await?;
    assert_eq!(
        bad,
        (403, r#"{"status":"error","error":"invalid_auth"}"#.into())
    );
    let (_, _, same) = get(h.addr, &ch_path).await?;
    assert!(same.contains("\"attempts_left\":2"));

    let init = sign(123, "test_token")?;
    let r1 = post_verify(h.addr, &h.token, wrong, &init, true).await?;
    assert_eq!(r1, (200, r#"{"status":"retry","remaining":1}"#.into()));
    let r_paths = paths(&h.mock).await?;
    assert!(
        !r_paths
            .iter()
            .any(|p| p.ends_with("/DeclineChatJoinRequest"))
    );

    let r2 = post_verify(h.addr, &h.token, wrong, &init, false).await?;
    assert_eq!(r2, (200, r#"{"status":"declined"}"#.into()));
    let routes = paths(&h.mock).await?;
    assert!(idx(&routes, "/EditMessageText")? < idx(&routes, "/DeclineChatJoinRequest")?);
    assert!(h.store.is_empty().await);
    h.stop().await;
    Ok(())
}
