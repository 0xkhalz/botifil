use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use botifil::{
    config::Config,
    domain::{Challenge, PendingJoin, SessionOutcome, SessionToken},
    handlers::join_request::handle_join_request,
    store::{MemoryStore, SessionStore},
    webapp::{INDEX_HTML, WebAppState, auth::sign_init_data_for_test, server::serve},
};
use teloxide::{
    Bot,
    types::{Chat, ChatId, ChatJoinRequest, ChatKind, ChatPrivate, MessageId, User, UserId},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinHandle,
    time::{Instant, sleep, timeout},
};
use url::Url;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

type Res<T = ()> = Result<T, Box<dyn std::error::Error>>;

const TEST_TOKEN: &str = "987654:test-secret-token";
const OK_MSG: &str =
    r#"{"ok":true,"result":{"message_id":42,"date":1,"chat":{"id":777,"type":"private"}}}"#;

struct HttpResp {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

struct TestApp {
    addr: SocketAddr,
    config: Arc<Config>,
    store: SessionStore,
    shutdown_tx: watch::Sender<bool>,
    server_task: JoinHandle<()>,
}

impl TestApp {
    async fn start(bot: Bot, webapp_url: Option<&str>) -> Res<Self> {
        let config = Arc::new(Config::from_all_env_vars(
            Some(TEST_TOKEN.into()),
            None,
            None,
            None,
            None,
            webapp_url,
            None,
        )?);
        let store = SessionStore::from_storage(Arc::new(MemoryStore::new(32)));
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let state = WebAppState {
            bot,
            config: Arc::clone(&config),
            store: store.clone(),
        };
        let server_task = tokio::spawn(serve(listener, state, shutdown_rx));
        Ok(Self {
            addr,
            config,
            store,
            shutdown_tx,
            server_task,
        })
    }

    async fn stop(self) -> Res {
        let _ = self.shutdown_tx.send(true);
        self.server_task.await?;
        Ok(())
    }
}

fn test_user(id: u64) -> User {
    User {
        id: UserId(id),
        is_bot: false,
        first_name: "Test".into(),
        last_name: None,
        username: None,
        language_code: None,
        is_premium: false,
        added_to_attachment_menu: false,
    }
}

#[allow(clippy::default_trait_access)]
fn join_request(user_id: u64, user_chat_id: i64) -> ChatJoinRequest {
    let kind = ChatKind::Private(ChatPrivate {
        first_name: None,
        last_name: None,
        username: None,
    });
    ChatJoinRequest {
        chat: Chat {
            id: ChatId(-100),
            kind,
        },
        from: test_user(user_id),
        user_chat_id: ChatId(user_chat_id),
        date: Default::default(),
        bio: None,
        invite_link: None,
    }
}

async fn start_telegram_mock() -> Res<(MockServer, Bot)> {
    let server = MockServer::start().await;
    let ok = r#"{"ok":true,"result":true}"#;
    for (name, body) in [
        ("SendMessage", OK_MSG),
        ("EditMessageText", OK_MSG),
        ("AnswerCallbackQuery", ok),
        ("ApproveChatJoinRequest", ok),
        ("DeclineChatJoinRequest", ok),
    ] {
        Mock::given(path(format!("/bot{TEST_TOKEN}/{name}")))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }
    let bot = Bot::new(TEST_TOKEN).set_api_url(Url::parse(&server.uri())?);
    Ok((server, bot))
}

fn parse_http_response(raw: &str) -> Res<HttpResp> {
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .ok_or("malformed HTTP response: missing header separator")?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or("missing status line")?;
    let status: u16 = status_line
        .split(' ')
        .nth(1)
        .ok_or("missing status code")?
        .parse()?;
    let mut headers = HashMap::new();
    for line in lines {
        let (k, v) = line.split_once(':').ok_or("malformed response header")?;
        headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_owned());
    }
    Ok(HttpResp {
        status,
        headers,
        body: body.to_owned(),
    })
}

async fn send_raw_http(addr: SocketAddr, raw: &[u8]) -> Res<HttpResp> {
    let mut stream = TcpStream::connect(addr).await?;
    stream.write_all(raw).await?;
    let mut buf = Vec::new();
    timeout(Duration::from_secs(10), stream.read_to_end(&mut buf)).await??;
    parse_http_response(&String::from_utf8(buf)?)
}

async fn http_get(addr: SocketAddr, target: &str) -> Res<HttpResp> {
    let req = format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n");
    send_raw_http(addr, req.as_bytes()).await
}

async fn http_post_form(addr: SocketAddr, target: &str, body: &str) -> Res<HttpResp> {
    let req = format!(
        "POST {target} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    send_raw_http(addr, req.as_bytes()).await
}

fn now_unix() -> Res<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn signed_init_data(user_id: u64, auth_date: u64) -> String {
    let user_json = format!(r#"{{"id":{user_id},"first_name":"Test"}}"#);
    let ts = auth_date.to_string();
    let raw = sign_init_data_for_test(&[("auth_date", &ts), ("user", &user_json)], TEST_TOKEN);
    url::form_urlencoded::byte_serialize(raw.as_bytes()).collect()
}

fn seed_session(
    token: &str,
    user_id: u64,
    challenge: Challenge,
    ttl: Duration,
) -> Res<PendingJoin> {
    Ok(PendingJoin {
        token: SessionToken::new(token)?,
        chat_id: ChatId(-100),
        user_id: UserId(user_id),
        user_chat_id: ChatId(777),
        challenge,
        attempts: 0,
        max_attempts: 2,
        message_id: Some(MessageId(42)),
        expires_at: Instant::now() + ttl,
    })
}

fn json_u8(body: &str, key: &str) -> Res<u8> {
    let pat = format!(r#""{key}":"#);
    let start = body.find(&pat).ok_or("missing json key")? + pat.len();
    let tail = &body[start..];
    let len = tail.bytes().take_while(u8::is_ascii_digit).count();
    Ok(tail[..len].parse()?)
}

fn json_choices(body: &str) -> Res<[u8; 3]> {
    let pat = r#""choices":["#;
    let start = body.find(pat).ok_or("missing choices")? + pat.len();
    let end = body[start..].find(']').ok_or("unterminated choices")?;
    let mut parts = body[start..start + end].split(',');
    let a: u8 = parts.next().ok_or("choice 0")?.trim().parse()?;
    let b: u8 = parts.next().ok_or("choice 1")?.trim().parse()?;
    let c: u8 = parts.next().ok_or("choice 2")?.trim().parse()?;
    Ok([a, b, c])
}

async fn received_paths(server: &MockServer) -> Res<Vec<String>> {
    let reqs = server.received_requests().await.ok_or("no requests")?;
    Ok(reqs.into_iter().map(|r| r.url.path().to_owned()).collect())
}

fn route_index(routes: &[String], suffix: &str) -> Res<usize> {
    routes
        .iter()
        .position(|r| r.ends_with(suffix))
        .ok_or_else(|| format!("missing route {suffix}").into())
}

fn extract_webapp_token(send_message_body: &str) -> Res<String> {
    let marker = "token=";
    let start = send_message_body
        .find(marker)
        .ok_or("missing token query parameter in web_app url")?
        + marker.len();
    let token: String = send_message_body[start..]
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect();
    if token.is_empty() {
        return Err("empty token in web_app url".into());
    }
    Ok(token)
}

#[tokio::test]
async fn e2e_get_root_serves_ui_html_with_security_headers() -> Res {
    let (_tg, bot) = start_telegram_mock().await?;
    let app = TestApp::start(bot, Some("https://example.com")).await?;

    let resp = http_get(app.addr, "/").await?;
    assert_eq!(resp.status, 200);
    assert_eq!(
        resp.headers.get("content-type").map(String::as_str),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(
        resp.headers.get("cache-control").map(String::as_str),
        Some("no-store")
    );
    assert_eq!(
        resp.headers
            .get("x-content-type-options")
            .map(String::as_str),
        Some("nosniff")
    );
    assert_eq!(resp.body, INDEX_HTML);

    let wrong_method = http_post_form(app.addr, "/", "").await?;
    assert_eq!(wrong_method.status, 405);
    assert!(
        wrong_method
            .body
            .contains(r#""error":"method_not_allowed""#)
    );

    app.stop().await
}

#[tokio::test]
async fn e2e_subpath_routing_matches_base_path_and_rejects_siblings() -> Res {
    let (_tg, bot) = start_telegram_mock().await?;
    let app = TestApp::start(bot, Some("https://example.com/botifil")).await?;
    let sess = seed_session(
        "toksub1",
        123,
        Challenge::new(2, 5, [7, 8, 9])?,
        Duration::from_secs(120),
    )?;
    app.store.upsert(sess, Instant::now()).await?;

    let bare = http_get(app.addr, "/botifil").await?;
    assert_eq!(bare.status, 200);
    assert_eq!(bare.body, INDEX_HTML);

    let slash = http_get(app.addr, "/botifil/").await?;
    assert_eq!(slash.status, 200);
    assert_eq!(slash.body, INDEX_HTML);

    let challenge = http_get(app.addr, "/botifil/api/challenge?token=toksub1").await?;
    assert_eq!(challenge.status, 200);
    assert!(challenge.body.contains(r#""left":2,"right":5"#));

    for not_found in [
        "/botifil2/",
        "/botifil2/api/challenge?token=toksub1",
        "/",
        "/api/challenge?token=toksub1",
    ] {
        let resp = http_get(app.addr, not_found).await?;
        assert_eq!(resp.status, 404, "expected 404 for {not_found}");
        assert!(resp.body.contains(r#""error":"not_found""#));
    }

    app.stop().await
}

#[tokio::test]
async fn e2e_get_challenge_handles_valid_invalid_and_expired_tokens() -> Res {
    let (_tg, bot) = start_telegram_mock().await?;
    let app = TestApp::start(bot, Some("https://example.com")).await?;
    let now = Instant::now();
    app.store
        .upsert(
            seed_session(
                "tokok1",
                123,
                Challenge::new(3, 4, [7, 9, 11])?,
                Duration::from_secs(120),
            )?,
            now,
        )
        .await?;
    app.store
        .upsert(
            seed_session(
                "tokexp1",
                456,
                Challenge::new(1, 2, [3, 4, 5])?,
                Duration::from_millis(10),
            )?,
            now,
        )
        .await?;
    sleep(Duration::from_millis(25)).await;

    let ok = http_get(app.addr, "/api/challenge?token=tokok1").await?;
    assert_eq!(ok.status, 200);
    assert_eq!(
        ok.headers.get("content-type").map(String::as_str),
        Some("application/json")
    );
    assert!(
        ok.body
            .contains(r#""left":3,"right":4,"choices":[7,9,11],"attempts_left":2"#)
    );
    assert!(ok.body.contains(r#""remaining_secs":"#));

    for bad_path in [
        "/api/challenge",
        "/api/challenge?token=",
        "/api/challenge?token=bad!token",
    ] {
        let resp = http_get(app.addr, bad_path).await?;
        assert_eq!(resp.status, 400, "expected 400 for {bad_path}");
        assert!(resp.body.contains(r#""error":"invalid_token""#));
    }

    for expired_path in [
        "/api/challenge?token=unknown123",
        "/api/challenge?token=tokexp1",
    ] {
        let resp = http_get(app.addr, expired_path).await?;
        assert_eq!(resp.status, 404, "expected 404 for {expired_path}");
        assert!(resp.body.contains(r#""status":"expired""#));
    }

    app.stop().await
}

#[tokio::test]
async fn e2e_post_verify_distinguishes_invalid_expired_and_wrong_user_auth() -> Res {
    let (_tg, bot) = start_telegram_mock().await?;
    let app = TestApp::start(bot, Some("https://example.com")).await?;
    app.store
        .upsert(
            seed_session(
                "tokauth",
                123,
                Challenge::new(4, 5, [9, 10, 11])?,
                Duration::from_secs(120),
            )?,
            Instant::now(),
        )
        .await?;

    let now_u = now_unix()?;
    let auth_wrong_user = signed_init_data(999, now_u);
    let auth_expired = signed_init_data(123, now_u.saturating_sub(600));

    let bad_auth = http_post_form(
        app.addr,
        "/api/verify",
        "token=tokauth&answer=9&init_data=tampered",
    )
    .await?;
    assert_eq!(bad_auth.status, 403);
    assert!(bad_auth.body.contains(r#""error":"invalid_auth""#));

    let exp_auth = http_post_form(
        app.addr,
        "/api/verify",
        &format!("token=tokauth&answer=9&init_data={auth_expired}"),
    )
    .await?;
    assert_eq!(exp_auth.status, 403);
    assert!(exp_auth.body.contains(r#""error":"invalid_auth""#));

    let wrong_user = http_post_form(
        app.addr,
        "/api/verify",
        &format!("token=tokauth&answer=9&init_data={auth_wrong_user}"),
    )
    .await?;
    assert_eq!(wrong_user.status, 403);
    assert!(wrong_user.body.contains(r#""error":"wrong_user""#));

    app.stop().await
}

#[tokio::test]
async fn e2e_post_verify_covers_retry_approve_decline_and_expired() -> Res {
    let (tg, bot) = start_telegram_mock().await?;
    let app = TestApp::start(bot, Some("https://example.com")).await?;
    let now_i = Instant::now();
    app.store
        .upsert(
            seed_session(
                "tokpass",
                123,
                Challenge::new(4, 5, [9, 10, 11])?,
                Duration::from_secs(120),
            )?,
            now_i,
        )
        .await?;
    app.store
        .upsert(
            seed_session(
                "tokfail",
                456,
                Challenge::new(6, 2, [8, 9, 10])?,
                Duration::from_secs(120),
            )?,
            now_i,
        )
        .await?;
    app.store
        .upsert(
            seed_session(
                "tokgone",
                789,
                Challenge::new(1, 1, [2, 3, 4])?,
                Duration::from_millis(10),
            )?,
            now_i,
        )
        .await?;
    sleep(Duration::from_millis(25)).await;

    let now_u = now_unix()?;
    let auth_123 = signed_init_data(123, now_u);
    let auth_456 = signed_init_data(456, now_u);

    let retry = http_post_form(
        app.addr,
        "/api/verify",
        &format!("token=tokpass&answer=10&init_data={auth_123}"),
    )
    .await?;
    assert_eq!(retry.status, 200);
    assert_eq!(retry.body, r#"{"status":"retry","remaining":1}"#);
    assert_eq!(received_paths(&tg).await?, Vec::<String>::new());

    let approved = http_post_form(
        app.addr,
        "/api/verify",
        &format!("token=tokpass&answer=9&init_data={auth_123}"),
    )
    .await?;
    assert_eq!(approved.status, 200);
    assert_eq!(approved.body, r#"{"status":"approved"}"#);

    let fail_retry = http_post_form(
        app.addr,
        "/api/verify",
        &format!("token=tokfail&answer=9&init_data={auth_456}"),
    )
    .await?;
    assert_eq!(fail_retry.status, 200);
    assert_eq!(fail_retry.body, r#"{"status":"retry","remaining":1}"#);

    let declined = http_post_form(
        app.addr,
        "/api/verify",
        &format!("token=tokfail&answer=10&init_data={auth_456}"),
    )
    .await?;
    assert_eq!(declined.status, 200);
    assert_eq!(declined.body, r#"{"status":"declined"}"#);

    for missing_tok in ["missing99", "tokgone"] {
        let resp = http_post_form(
            app.addr,
            "/api/verify",
            &format!("token={missing_tok}&answer=2&init_data={auth_123}"),
        )
        .await?;
        assert_eq!(resp.status, 404);
        assert_eq!(resp.body, r#"{"status":"expired"}"#);
    }

    app.stop().await
}

#[tokio::test]
async fn e2e_http_hardening_rejects_chunked_duplicate_length_and_timeout() -> Res {
    let (_tg, bot) = start_telegram_mock().await?;
    let app = TestApp::start(bot, Some("https://example.com")).await?;

    let chunked = send_raw_http(
        app.addr,
        b"POST /api/verify HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
    )
    .await?;
    assert_eq!(chunked.status, 400);
    assert!(chunked.body.contains(r#""error":"bad_request""#));

    let dup_len = send_raw_http(
        app.addr,
        b"POST /api/verify HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\nContent-Length: 5\r\n\r\nhello",
    )
    .await?;
    assert_eq!(dup_len.status, 400);
    assert!(dup_len.body.contains(r#""error":"bad_request""#));

    let mut slow_stream = TcpStream::connect(app.addr).await?;
    slow_stream
        .write_all(
            b"POST /api/verify HTTP/1.1\r\nHost: localhost\r\nContent-Length: 20\r\n\r\npart",
        )
        .await?;
    let mut raw_timeout_resp = Vec::new();
    timeout(
        Duration::from_secs(10),
        slow_stream.read_to_end(&mut raw_timeout_resp),
    )
    .await??;
    let timed_out = parse_http_response(&String::from_utf8(raw_timeout_resp)?)?;
    assert_eq!(timed_out.status, 408);
    assert!(timed_out.body.contains(r#""error":"request_timeout""#));

    app.stop().await
}

#[tokio::test]
async fn e2e_join_request_webapp_flow_enforces_telegram_api_ordering() -> Res {
    let (tg_ok, bot_ok) = start_telegram_mock().await?;
    let app_ok = TestApp::start(bot_ok.clone(), Some("https://example.com/botifil")).await?;

    let out = handle_join_request(
        bot_ok,
        join_request(123, 777),
        Arc::clone(&app_ok.config),
        app_ok.store.clone(),
    )
    .await?;
    assert_eq!(out, SessionOutcome::ChallengeSent);

    let sent_reqs = tg_ok.received_requests().await.ok_or("no requests")?;
    assert_eq!(sent_reqs.len(), 1);
    let send_body = std::str::from_utf8(&sent_reqs[0].body)?;
    assert!(send_body.contains("web_app"));
    assert!(send_body.contains("https://example.com/botifil?token="));
    assert!(!send_body.contains("callback_data"));

    let token = extract_webapp_token(send_body)?;
    let ch_resp = http_get(
        app_ok.addr,
        &format!("/botifil/api/challenge?token={token}"),
    )
    .await?;
    assert_eq!(ch_resp.status, 200);
    let correct = json_u8(&ch_resp.body, "left")? + json_u8(&ch_resp.body, "right")?;

    let auth_ok = signed_init_data(123, now_unix()?);
    let verify_ok = http_post_form(
        app_ok.addr,
        "/botifil/api/verify",
        &format!("token={token}&answer={correct}&init_data={auth_ok}"),
    )
    .await?;
    assert_eq!(verify_ok.status, 200);
    assert_eq!(verify_ok.body, r#"{"status":"approved"}"#);

    let ok_routes = received_paths(&tg_ok).await?;
    assert!(
        route_index(&ok_routes, "/ApproveChatJoinRequest")?
            < route_index(&ok_routes, "/EditMessageText")?
    );
    app_ok.stop().await?;

    let (tg_decline, bot_decline) = start_telegram_mock().await?;
    let app_decline =
        TestApp::start(bot_decline.clone(), Some("https://example.com/botifil")).await?;
    let out_decline = handle_join_request(
        bot_decline,
        join_request(456, 777),
        Arc::clone(&app_decline.config),
        app_decline.store.clone(),
    )
    .await?;
    assert_eq!(out_decline, SessionOutcome::ChallengeSent);

    let dec_reqs = tg_decline.received_requests().await.ok_or("no requests")?;
    let dec_token = extract_webapp_token(std::str::from_utf8(&dec_reqs[0].body)?)?;
    let dec_ch = http_get(
        app_decline.addr,
        &format!("/botifil/api/challenge?token={dec_token}"),
    )
    .await?;
    let dec_sum = json_u8(&dec_ch.body, "left")? + json_u8(&dec_ch.body, "right")?;
    let wrong_choice = json_choices(&dec_ch.body)?
        .into_iter()
        .find(|&c| c != dec_sum)
        .ok_or("missing wrong choice")?;

    let auth_456 = signed_init_data(456, now_unix()?);
    let retry_resp = http_post_form(
        app_decline.addr,
        "/botifil/api/verify",
        &format!("token={dec_token}&answer={wrong_choice}&init_data={auth_456}"),
    )
    .await?;
    assert_eq!(retry_resp.status, 200);
    assert_eq!(retry_resp.body, r#"{"status":"retry","remaining":1}"#);

    let final_resp = http_post_form(
        app_decline.addr,
        "/botifil/api/verify",
        &format!("token={dec_token}&answer={wrong_choice}&init_data={auth_456}"),
    )
    .await?;
    assert_eq!(final_resp.status, 200);
    assert_eq!(final_resp.body, r#"{"status":"declined"}"#);

    let dec_routes = received_paths(&tg_decline).await?;
    assert!(
        route_index(&dec_routes, "/DeclineChatJoinRequest")?
            < route_index(&dec_routes, "/EditMessageText")?
    );
    app_decline.stop().await
}

#[tokio::test]
async fn e2e_join_request_falls_back_to_three_button_math_when_webapp_unset() -> Res {
    let (tg, bot) = start_telegram_mock().await?;
    let cfg = Arc::new(Config::from_token(Some(TEST_TOKEN.into()))?);
    let store = SessionStore::from_storage(Arc::new(MemoryStore::new(8)));

    let out = handle_join_request(bot, join_request(123, 777), cfg, store).await?;
    assert_eq!(out, SessionOutcome::ChallengeSent);

    let reqs = tg.received_requests().await.ok_or("no requests")?;
    assert_eq!(reqs.len(), 1);
    let body = std::str::from_utf8(&reqs[0].body)?;
    assert!(!body.contains("web_app"));
    assert!(body.contains("Verify you are human: what is "));
    assert_eq!(body.matches("\"callback_data\"").count(), 3);
    Ok(())
}
