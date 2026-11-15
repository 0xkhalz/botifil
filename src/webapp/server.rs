use std::{
    borrow::Cow,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use teloxide::{RequestError, requests::Requester};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};
use tokio::time::{Instant, timeout, timeout_at};

use crate::config::Config;
use crate::domain::SessionToken;
use crate::error::classify_request_error;
use crate::handlers::callback::resolve_session;
use crate::store::{AttemptOutcome, SessionStore};
use crate::webapp::UI_HTML;
use crate::webapp::auth::{percent_decode, verify_telegram_init_data};

const MAX_CONNS: u16 = 64;
const MAX_LIMIT: usize = 16 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const JSON_CT: &str = "application/json; charset=utf-8";
const CSP: &str = "Content-Security-Policy: default-src 'none'; script-src 'unsafe-inline' https://telegram.org; style-src 'unsafe-inline'; connect-src 'self'\r\n";

struct State<B>(B, Arc<Config>, SessionStore);
type Resp = (&'static str, &'static str, &'static str, Cow<'static, str>);

#[rustfmt::skip]
fn json(st: &'static str, body: impl Into<Cow<'static, str>>) -> Resp { (st, JSON_CT, "", body.into()) }
#[rustfmt::skip]
fn err(st: &'static str, code: &str) -> Resp { json(st, format!(r#"{{"error":"{code}"}}"#)) }
#[rustfmt::skip]
fn verr(st: &'static str, code: &str) -> Resp { json(st, format!(r#"{{"status":"error","error":"{code}"}}"#)) }

async fn read_chunk(
    stream: &mut TcpStream,
    dl: Instant,
    buf: &mut [u8],
) -> Result<usize, Option<Resp>> {
    timeout_at(dl, stream.read(buf))
        .await
        .map_err(|_| Some(err("408 Request Timeout", "request_timeout")))?
        .map_err(|_| None)
}

#[rustfmt::skip]
fn parse_head(raw: &[u8]) -> Result<(String, String, usize), Resp> {
    let bad = || err("400 Bad Request", "bad_request");
    let mut lines = std::str::from_utf8(raw).map_err(|_| bad())?.split("\r\n");
    let mut parts = lines.next().ok_or_else(bad)?.split_ascii_whitespace();
    let (Some(meth), Some(tgt), Some(ver), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return Err(bad()); };
    if !ver.starts_with("HTTP/1.") { return Err(bad()); }
    let mut content_len = None;
    for line in lines {
        let (hdr, val) = line.split_once(':').ok_or_else(bad)?;
        let (hdr, val) = (hdr.trim(), val.trim());
        if hdr.eq_ignore_ascii_case("transfer-encoding") { return Err(bad()); }
        if hdr.eq_ignore_ascii_case("content-length") {
            let len: usize = val.parse().map_err(|_| bad())?;
            if content_len.replace(len).is_some() { return Err(bad()); }
            if len > MAX_LIMIT { return Err(err("413 Payload Too Large", "payload_too_large")); }
        }
    }
    Ok((meth.to_owned(), tgt.to_owned(), content_len.unwrap_or(0)))
}

#[rustfmt::skip]
async fn read_request(stream: &mut TcpStream) -> Result<(String, String, Vec<u8>), Option<Resp>> {
    let (dl, mut chunk, mut buf) = (Instant::now() + IO_TIMEOUT, [0_u8; 4096], Vec::new());
    let hdr_end = loop {
        if let Some(pos) = buf.windows(4).position(|win| win == b"\r\n\r\n") {
            if pos + 4 > MAX_LIMIT { return Err(Some(err("413 Payload Too Large", "payload_too_large"))); }
            break pos + 4;
        }
        if buf.len() >= MAX_LIMIT { return Err(Some(err("413 Payload Too Large", "payload_too_large"))); }
        let read_n = read_chunk(stream, dl, &mut chunk).await?;
        if read_n == 0 { return Err((!buf.is_empty()).then(|| err("400 Bad Request", "bad_request"))); }
        buf.extend_from_slice(&chunk[..read_n]);
    };
    let (method, target, target_len) = parse_head(&buf[..hdr_end - 4]).map_err(Some)?;
    let mut body = buf.split_off(hdr_end);
    body.truncate(target_len);
    while body.len() < target_len {
        let read_n = read_chunk(stream, dl, &mut chunk).await?;
        if read_n == 0 { return Err(Some(err("400 Bad Request", "bad_request"))); }
        body.extend_from_slice(&chunk[..read_n.min(target_len - body.len())]);
    }
    Ok((method, target, body))
}

#[rustfmt::skip]
fn parse_pairs(raw: &str) -> Option<Vec<(String, String)>> {
    raw.split('&').filter(|part| !part.is_empty()).map(|part| {
        let (key, val) = part.split_once('=').unwrap_or((part, ""));
        Some((percent_decode(key).ok()?, percent_decode(val).ok()?))
    }).collect()
}

#[rustfmt::skip]
fn field<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> { pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()) }

#[rustfmt::skip]
async fn handle_challenge(query: &str, store: &SessionStore) -> Resp {
    let tok = parse_pairs(query).and_then(|ps| SessionToken::new(field(&ps, "token")?).ok());
    let Some(tok) = tok else { return err("400 Bad Request", "invalid_token"); };
    let now = Instant::now();
    let Some(sess) = store.get_by_token(&tok, now).await else { return err("404 Not Found", "expired_or_not_found"); };
    let rem = sess.expires_at.saturating_duration_since(now).as_secs().max(1);
    let left = sess.max_attempts.saturating_sub(sess.attempts);
    let [c0, c1, c2] = sess.challenge.choices;
    json("200 OK", format!("{{\"left\":{},\"right\":{},\"choices\":[{c0},{c1},{c2}],\"remaining_secs\":{rem},\"attempts_left\":{left}}}", sess.challenge.left, sess.challenge.right))
}

#[rustfmt::skip]
async fn handle_verify<B: Requester<Err = RequestError>>(body: &[u8], st: &State<B>) -> Resp {
    let Some(pairs) = std::str::from_utf8(body).ok().and_then(parse_pairs) else { return verr("400 Bad Request", "bad_request"); };
    let Some(tok) = field(&pairs, "token").and_then(|raw| SessionToken::new(raw).ok()) else { return verr("400 Bad Request", "invalid_token"); };
    let Some(ans) = field(&pairs, "answer").and_then(|raw| raw.parse::<u8>().ok()) else { return verr("400 Bad Request", "invalid_answer"); };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |dur| dur.as_secs());
    let uid = match verify_telegram_init_data(field(&pairs, "init_data").unwrap_or(""), st.1.telegram_token(), now, st.1.challenge_ttl().as_secs()) {
        Ok(uid) => uid,
        Err(why) => { tracing::warn!(reason = %why, "Rejected WebApp initData"); return verr("403 Forbidden", "invalid_auth"); }
    };
    let out = st.2.verify_attempt(&tok, uid, ans, Instant::now()).await;
    let pass = matches!(out, AttemptOutcome::Approved(_));
    match out {
        AttemptOutcome::WrongUser => verr("403 Forbidden", "wrong_user"),
        AttemptOutcome::NotFoundOrExpired => json("404 Not Found", r#"{"status":"expired"}"#),
        AttemptOutcome::Retry { remaining: rem } => json("200 OK", format!(r#"{{"status":"retry","remaining":{rem}}}"#)),
        AttemptOutcome::Approved(sess) | AttemptOutcome::DeclinedExhausted(sess) => match resolve_session(&st.0, &sess, pass).await {
            Ok(_) => json("200 OK", if pass { r#"{"status":"approved"}"# } else { r#"{"status":"declined"}"# }),
            Err(api_err) => {
                tracing::error!(action = ?classify_request_error(&api_err), "Telegram API failure resolving WebApp");
                verr("502 Bad Gateway", "upstream_error")
            }
        },
    }
}

#[rustfmt::skip]
async fn handle_conn<B: Requester<Err = RequestError>>(mut stream: TcpStream, st: Arc<State<B>>) {
    let resp = match read_request(&mut stream).await {
        Ok((meth, tgt, body)) => match (meth.as_str(), tgt.split_once('?').unwrap_or((&tgt, ""))) {
            ("GET", ("/", _)) => ("200 OK", "text/html; charset=utf-8", CSP, Cow::Borrowed(UI_HTML)),
            ("GET", ("/api/challenge", query)) => handle_challenge(query, &st.2).await,
            ("POST", ("/api/verify", _)) => handle_verify(&body, &st).await,
            _ => err("404 Not Found", "not_found"),
        },
        Err(Some(resp)) => resp,
        Err(None) => return,
    };
    let (status, ct, extra, body) = resp;
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n{extra}Connection: close\r\n\r\n", body.len());
    let _ = timeout(IO_TIMEOUT, async {
        stream.write_all(head.as_bytes()).await?;
        stream.write_all(body.as_bytes()).await?;
        stream.flush().await?;
        stream.shutdown().await
    }).await;
}

/// Runs the embedded HTTP/1.1 `WebApp` server until `shutdown` becomes `true`.
pub async fn serve_webapp<B>(
    listener: TcpListener,
    bot: B,
    config: Arc<Config>,
    store: SessionStore,
    mut shutdown: watch::Receiver<bool>,
) where
    B: Requester<Err = RequestError> + Clone + Send + Sync + 'static,
{
    let sem = Arc::new(Semaphore::new(usize::from(MAX_CONNS)));
    let st = Arc::new(State(bot, config, store));
    while !*shutdown.borrow() {
        tokio::select! {
            res = shutdown.changed() => if res.is_err() || *shutdown.borrow() { break; },
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let Ok(permit) = sem.clone().try_acquire_owned() else {
                        tracing::warn!("WebApp connection limit reached; shedding connection");
                        continue;
                    };
                    let st_ref = Arc::clone(&st);
                    tokio::spawn(async move { handle_conn(stream, st_ref).await; drop(permit); });
                }
                Err(accept_err) => tracing::warn!(error = %accept_err, "WebApp accept error"),
            }
        }
    }
    let _ = timeout(IO_TIMEOUT, sem.acquire_many(u32::from(MAX_CONNS))).await;
}
