use crate::domain::{SessionOutcome, SessionToken};
use crate::webapp::{INDEX_HTML, WebAppState, auth, resolve_join_request};
use crate::{config::Config, error::classify_request_error, store::AttemptOutcome};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use teloxide::{RequestError, requests::Requester};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::time::{Instant, timeout_at};

const MAX_HEADER_BYTES: usize = 8192;
const MAX_BODY_BYTES: usize = 16_384;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const ACCEPT_BACKOFF: Duration = Duration::from_millis(50);
const HTML: &str = "text/html; charset=utf-8";
type Resp = (u16, &'static str, Vec<u8>);

fn json(st: u16, body: impl Into<Vec<u8>>) -> Resp {
    (st, "application/json", body.into())
}
fn err(st: u16, code: &str) -> Resp {
    json(st, format!(r#"{{"error":"{code}"}}"#))
}

#[rustfmt::skip]
const fn reason(st: u16) -> &'static str {
    match st { 200 => "OK", 400 => "Bad Request", 403 => "Forbidden", 404 => "Not Found", 405 => "Method Not Allowed", 408 => "Request Timeout", 500 => "Internal Server Error", 502 => "Bad Gateway", _ => "Unknown" }
}

fn strip_base<'a>(cfg: &Config, raw: &'a str) -> Option<&'a str> {
    let b = cfg
        .webapp_url()
        .map_or("", |u| u.path().trim_end_matches('/'));
    if b.is_empty() {
        return raw.starts_with('/').then_some(raw);
    }
    if raw == b {
        Some("/")
    } else {
        raw.strip_prefix(b).filter(|r| r.starts_with('/'))
    }
}

fn parse_headers(raw: &[u8]) -> Option<(String, String, usize)> {
    let mut lines = std::str::from_utf8(raw).ok()?.split("\r\n");
    let mut rl = lines.next()?.split(' ');
    let (meth, target, ver, None) = (rl.next()?, rl.next()?, rl.next()?, rl.next()) else {
        return None;
    };
    let ok_m = !meth.is_empty() && meth.bytes().all(|b| b.is_ascii_alphabetic());
    let ok_t = target.starts_with('/') && !target.bytes().any(|b| b.is_ascii_control());
    if !ok_m || !ok_t || !matches!(ver, "HTTP/1.1" | "HTTP/1.0") {
        return None;
    }
    let mut len = None;
    for line in lines {
        let (key, val) = line.split_once(':')?;
        let bad_k = key.is_empty() || key.bytes().any(|b| b.is_ascii_whitespace());
        if bad_k || key.eq_ignore_ascii_case("transfer-encoding") {
            return None;
        }
        if key.eq_ignore_ascii_case("content-length") {
            let raw_len = val.trim();
            if len.is_some() || raw_len.is_empty() || !raw_len.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            len = Some(
                raw_len
                    .parse::<usize>()
                    .ok()
                    .filter(|&n| n <= MAX_BODY_BYTES)?,
            );
        }
    }
    Some((meth.into(), target.into(), len.unwrap_or(0)))
}

async fn read_request<R: AsyncRead + Unpin>(
    s: &mut R,
    lim: Duration,
) -> Result<(String, String, Vec<u8>), u16> {
    let (mut buf, mut tmp, dl) = (Vec::with_capacity(1024), [0_u8; 2048], Instant::now() + lim);
    let hdr_end = loop {
        let end = buf.len().min(MAX_HEADER_BYTES);
        if let Some(pos) = buf[..end].windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() >= MAX_HEADER_BYTES {
            return Err(400);
        }
        match timeout_at(dl, s.read(&mut tmp)).await {
            Err(_) => return Err(408),
            Ok(Ok(n)) if n > 0 => buf.extend_from_slice(&tmp[..n]),
            _ => return Err(400),
        }
    };
    let (m, tgt, blen) = parse_headers(&buf[..hdr_end]).ok_or(400_u16)?;
    let (mut body, extra) = (vec![0_u8; blen], &buf[hdr_end + 4..]);
    let have = extra.len().min(blen);
    body[..have].copy_from_slice(&extra[..have]);
    if have < blen {
        let r = timeout_at(dl, s.read_exact(&mut body[have..])).await;
        r.map_err(|_| 408_u16)?.map_err(|_| 400_u16)?;
    }
    Ok((m, tgt, body))
}

async fn handle_challenge<B>(query: &str, state: &WebAppState<B>) -> Resp {
    let raw = url::form_urlencoded::parse(query.as_bytes()).find(|(k, _)| k == "token");
    let Ok(tok) = SessionToken::new(raw.map_or_default(|(_, v)| v.into_owned())) else {
        return err(400, "invalid_token");
    };
    let now = Instant::now();
    match state.store.get_by_token(&tok, now).await {
        Err(e) => {
            tracing::warn!(error = %e, "Failed to read challenge session");
            err(500, "internal_error")
        }
        Ok(None) => json(404, br#"{"status":"expired"}"#),
        Ok(Some(s)) => {
            let left = s.max_attempts.saturating_sub(s.attempts);
            let rem = s.expires_at.saturating_duration_since(now).as_secs();
            let (l, r, ch) = (s.challenge.left, s.challenge.right, s.challenge.choices);
            #[rustfmt::skip]
            let b = format!(r#"{{"left":{l},"right":{r},"choices":[{},{},{}],"attempts_left":{left},"remaining_secs":{rem}}}"#, ch[0], ch[1], ch[2]);
            json(200, b)
        }
    }
}

async fn handle_verify<B: Requester<Err = RequestError>>(body: &[u8], st: &WebAppState<B>) -> Resp {
    let form: HashMap<_, _> = url::form_urlencoded::parse(body).collect();
    let get = |k| form.get(k).map_or("", AsRef::as_ref);
    let Ok(tok) = SessionToken::new(get("token")) else {
        return err(400, "invalid_token");
    };
    let raw_a = get("answer");
    let ok_a = !raw_a.is_empty() && raw_a.bytes().all(|b| b.is_ascii_digit());
    let Some(ans) = ok_a.then(|| raw_a.parse().ok()).flatten() else {
        return err(400, "invalid_answer");
    };
    #[rustfmt::skip]
    let now_u = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let ttl = st.config.challenge_ttl().as_secs();
    let uid =
        auth::verify_telegram_init_data(get("init_data"), st.config.telegram_token(), now_u, ttl);
    let Ok(uid) = uid else {
        return err(403, "invalid_auth");
    };
    let out = st
        .store
        .verify_attempt(&tok, uid, ans, Instant::now())
        .await;
    let (sess, pass) = match out {
        AttemptOutcome::WrongUser => return err(403, "wrong_user"),
        AttemptOutcome::NotFoundOrExpired => return json(404, br#"{"status":"expired"}"#),
        AttemptOutcome::Retry { remaining } => {
            #[rustfmt::skip]
            return json(200, format!(r#"{{"status":"retry","remaining":{remaining}}}"#));
        }
        AttemptOutcome::Approved(s) => (s, true),
        AttemptOutcome::DeclinedExhausted(s) => (s, false),
    };
    match resolve_join_request(&st.bot, &sess, pass).await {
        Ok(SessionOutcome::IdempotentNoOp) => json(404, br#"{"status":"expired"}"#),
        Ok(SessionOutcome::Approved | SessionOutcome::IdempotentAlreadyMember) => {
            json(200, br#"{"status":"approved"}"#)
        }
        Ok(_) => json(200, br#"{"status":"declined"}"#),
        Err(e) => {
            tracing::warn!(action = ?classify_request_error(&e), "Telegram API error during webapp verification");
            err(502, "telegram_error")
        }
    }
}

async fn handle_conn<B>(mut stream: TcpStream, state: WebAppState<B>)
where
    B: Requester<Err = RequestError> + Clone + Send + Sync + 'static,
{
    let (st, ct, body) = match read_request(&mut stream, READ_TIMEOUT).await {
        Err(408) => err(408, "request_timeout"),
        Err(c) => err(c, "bad_request"),
        Ok((m, tgt, req_body)) => {
            let (raw_path, q) = tgt.split_once('?').unwrap_or((&tgt, ""));
            match (m.as_str(), strip_base(&state.config, raw_path)) {
                ("GET", Some("/")) => (200, HTML, INDEX_HTML.as_bytes().to_vec()),
                ("GET", Some("/api/challenge")) => handle_challenge(q, &state).await,
                ("POST", Some("/api/verify")) => handle_verify(&req_body, &state).await,
                (_, Some("/" | "/api/challenge" | "/api/verify")) => err(405, "method_not_allowed"),
                _ => err(404, "not_found"),
            }
        }
    };
    #[rustfmt::skip]
    let hdr = format!("HTTP/1.1 {st} {}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n", reason(st), body.len());
    let _ = stream.write_all(&[hdr.as_bytes(), &body].concat()).await;
    let _ = stream.shutdown().await;
}

/// Runs the embedded HTTP/1.1 verification `WebApp` server until `shutdown` signals `true`.
pub async fn serve<B>(
    listener: TcpListener,
    state: WebAppState<B>,
    mut shutdown: watch::Receiver<bool>,
) where
    B: Requester<Err = RequestError> + Clone + Send + Sync + 'static,
{
    while !*shutdown.borrow() {
        tokio::select! {
            biased;
            ch = shutdown.changed() => if ch.is_err() || *shutdown.borrow() { break },
            res = listener.accept() => match res {
                Ok((stream, _)) => { let st = state.clone(); tokio::spawn(async move { handle_conn(stream, st).await }); }
                Err(e) => {
                    tracing::warn!(error = %e, fd_exhausted = matches!(e.raw_os_error(), Some(24 | 23)), "WebApp accept error");
                    tokio::time::sleep(ACCEPT_BACKOFF).await;
                }
            }
        }
    }
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use super::*;
    use crate::{domain::Challenge, store::{MemoryStore, SessionStore}, webapp::auth::sign_init_data_for_test};
    use std::{net::SocketAddr, sync::Arc};
    use teloxide::{Bot, types::{ChatId, UserId}};
    type Res = Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn subpath_stripping() -> Res {
        let cfg = |u| Config::from_all_env_vars(Some("1:t".into()), None, None, None, None, u, None);
        let (c0, c1, c2) = (cfg(None)?, cfg(Some("https://x.io/"))?, cfg(Some("https://x.io/bot/"))?);
        for (c, r, w) in [(&c0, "/", Some("/")), (&c1, "/api/challenge", Some("/api/challenge")), (&c2, "/bot", Some("/")), (&c2, "/bot/", Some("/")), (&c2, "/bot/api/challenge", Some("/api/challenge")), (&c2, "/bot2", None), (&c2, "/", None)] {
            assert_eq!(strip_base(c, r), w);
        }
        Ok(())
    }
    #[tokio::test]
    async fn http_hardening_rejects_bad_inputs_and_timeouts() -> Res {
        let big = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(MAX_HEADER_BYTES));
        for raw in [b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n".as_slice(), b"POST /api/verify HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 5\r\n\r\nhello", b"POST /api/verify HTTP/1.1\r\nContent-Length: +5\r\n\r\nhello", b"POST /api/verify HTTP/1.1\r\nContent-Length: 99999\r\n\r\n", b"BAD_LINE\r\n\r\n", b"GET / HTTP/1.1\r\nHost: a", big.as_bytes()] {
            let (mut rd, mut wr) = tokio::io::duplex(20_000);
            wr.write_all(raw).await?; drop(wr);
            assert_eq!(read_request(&mut rd, READ_TIMEOUT).await, Err(400));
        }
        let (mut rd, _wr) = tokio::io::duplex(64);
        assert_eq!(read_request(&mut rd, Duration::from_millis(20)).await, Err(408));
        Ok(())
    }
    async fn http(addr: SocketAddr, req: &str) -> Result<(u16, String), std::io::Error> {
        let mut s = TcpStream::connect(addr).await?;
        s.write_all(req.as_bytes()).await?;
        let mut out = String::new(); s.read_to_string(&mut out).await?;
        let st = out.lines().next().and_then(|l| l.split(' ').nth(1));
        Ok((st.and_then(|c| c.parse().ok()).unwrap_or(0), out))
    }
    #[tokio::test]
    async fn routes_and_graceful_shutdown() -> Res {
        let t = "987654:test-secret-token";
        let cfg = Config::from_all_env_vars(Some(t.into()), None, None, None, None, Some("https://x.io/app"), None)?;
        let store = SessionStore::from_storage(Arc::new(MemoryStore::new(16)));
        let sess = crate::domain::PendingJoin { token: SessionToken::new("tok123")?, chat_id: ChatId(-999), user_id: UserId(42), user_chat_id: ChatId(42), challenge: Challenge::new(2, 3, [5, 6, 7])?, attempts: 0, max_attempts: 2, message_id: None, expires_at: Instant::now() + Duration::from_secs(120) };
        store.upsert(sess, Instant::now()).await?;
        let (ln, (tx, rx)) = (TcpListener::bind("127.0.0.1:0").await?, watch::channel(false));
        let (addr, st) = (ln.local_addr()?, WebAppState { bot: Bot::new(t), config: Arc::new(cfg), store });
        let srv = tokio::spawn(serve(ln, st, rx));
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().to_string();
        let sign = |id| url::form_urlencoded::byte_serialize(sign_init_data_for_test(&[("auth_date", &now), ("user", id)], t).as_bytes()).collect::<String>();
        let (a_ok, a_bad) = (sign(r#"{"id":42}"#), sign(r#"{"id":77}"#));
        let get = |p: &str| format!("GET {p} HTTP/1.1\r\nHost: x\r\n\r\n"); let post = |b: &str| format!("POST /app/api/verify HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{b}", b.len());
        for (req, code, pat) in [
            (get("/app"), 200, "Human Verification"), ("POST /app HTTP/1.1\r\nHost: x\r\n\r\n".into(), 405, "method_not_allowed"),
            (get("/other"), 404, "not_found"), (get("/app/api/challenge?token=bad!"), 400, "invalid_token"),
            (get("/app/api/challenge?token=none"), 404, "expired"), (get("/app/api/challenge?token=tok123"), 200, r#""left":2,"right":3"#),
            (post("token=tok123&answer=6&init_data=bad"), 403, "invalid_auth"), (post(&format!("token=tok123&answer=6&init_data={a_bad}")), 403, "wrong_user"),
            (post(&format!("token=tok123&answer=6&init_data={a_ok}")), 200, r#""remaining":1"#), (post(&format!("token=missing&answer=5&init_data={a_ok}")), 404, "expired"),
        ] {
            let (got, resp) = http(addr, &req).await?;
            assert!(got == code && resp.contains(pat) && !resp.contains("-999") && !resp.contains(t));
        }
        tx.send(true)?; srv.await?; Ok(())
    }
}
