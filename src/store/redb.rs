use super::AttemptOutcome::{Approved, DeclinedExhausted, NotFoundOrExpired, Retry, WrongUser};
use super::{AttemptOutcome, Storage, StoreError};
use crate::domain::{Challenge, PendingJoin, SessionToken};
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use teloxide::types::{ChatId, MessageId, UserId};
use tokio::time::Instant;

type Row = [u8; 44];
type Res<T> = Result<T, StoreError>;
const TABLE: TableDefinition<&str, Row> = TableDefinition::new("pending_joins");
type Table<'a> = redb::Table<'a, &'static str, Row>;

pub struct RedbSessionStore {
    db: Database,
    max_capacity: usize,
    anchor: Instant,
    anchor_ms: u64,
}

fn err(e: impl std::fmt::Display) -> StoreError {
    StoreError::Backend(e.to_string())
}
fn or_log<R>(res: Res<R>, fb: R) -> R {
    res.unwrap_or_else(|e| {
        tracing::error!(error = %e, "redb operation failed");
        fb
    })
}
fn dur_ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}
fn le<const N: usize>(buf: &Row, off: usize) -> [u8; N] {
    buf.get(off..off + N)
        .map_or([0; N], |s| s.try_into().unwrap_or([0; N]))
}
fn is_peer(buf: &Row, chat: ChatId, user: UserId) -> bool {
    i64::from_le_bytes(le(buf, 0)) == chat.0 && u64::from_le_bytes(le(buf, 8)) == user.0
}

fn encode(pj: &PendingJoin, exp_ms: u64) -> Row {
    let (mut buf, ch) = ([0u8; 44], &pj.challenge);
    let [c0, c1, c2] = ch.choices;
    buf[0..8].copy_from_slice(&pj.chat_id.0.to_le_bytes());
    buf[8..16].copy_from_slice(&pj.user_id.0.to_le_bytes());
    buf[16..24].copy_from_slice(&pj.user_chat_id.0.to_le_bytes());
    buf[24..31].copy_from_slice(&[ch.left, ch.right, c0, c1, c2, pj.attempts, pj.max_attempts]);
    buf[31] = u8::from(pj.message_id.is_some());
    buf[32..36].copy_from_slice(&pj.message_id.map_or(0, |m| m.0).to_le_bytes());
    buf[36..44].copy_from_slice(&exp_ms.to_le_bytes());
    buf
}

impl RedbSessionStore {
    /// Opens or creates a `redb`-backed session store at `path`.
    ///
    /// # Errors
    /// Returns [`StoreError::Backend`] if the database cannot be opened or initialized.
    pub fn open(path: impl AsRef<std::path::Path>, max_capacity: usize) -> Res<Self> {
        let p = path.as_ref();
        if let Some(dir) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let sys = SystemTime::now().duration_since(UNIX_EPOCH);
        let st = Self {
            db: Database::create(p).map_err(err)?,
            max_capacity,
            anchor: Instant::now(),
            anchor_ms: dur_ms(sys.unwrap_or_default()),
        };
        st.write_tx(|_| Ok((true, ())))?;
        Ok(st)
    }
    fn to_ms(&self, inst: Instant) -> u64 {
        let fwd = dur_ms(inst.saturating_duration_since(self.anchor));
        let back = dur_ms(self.anchor.saturating_duration_since(inst));
        self.anchor_ms.saturating_add(fwd).saturating_sub(back)
    }
    fn to_instant(&self, ms: u64) -> Instant {
        let d = Duration::from_millis(ms.abs_diff(self.anchor_ms));
        if ms < self.anchor_ms {
            return self.anchor.checked_sub(d).unwrap_or(self.anchor);
        }
        self.anchor + d
    }
    fn decode(&self, tok: &str, buf: &Row) -> Res<PendingJoin> {
        let message_id = match buf[31] {
            0 => None,
            1 => Some(MessageId(i32::from_le_bytes(le(buf, 32)))),
            tag => return Err(StoreError::Backend(format!("bad msg tag: {tag}"))),
        };
        Ok(PendingJoin {
            token: SessionToken::new(tok).map_err(err)?,
            chat_id: ChatId(i64::from_le_bytes(le(buf, 0))),
            user_id: UserId(u64::from_le_bytes(le(buf, 8))),
            user_chat_id: ChatId(i64::from_le_bytes(le(buf, 16))),
            challenge: Challenge::new(buf[24], buf[25], [buf[26], buf[27], buf[28]])
                .map_err(err)?,
            attempts: buf[29],
            max_attempts: buf[30],
            message_id,
            expires_at: self.to_instant(u64::from_le_bytes(le(buf, 36))),
        })
    }
    fn take_where(&self, tbl: &mut Table<'_>, p: impl Fn(&Row) -> bool) -> Res<Vec<PendingJoin>> {
        let mut hits = Vec::new();
        for entry in tbl.iter().map_err(err)? {
            let (k, v) = entry.map_err(err)?;
            if p(&v.value()) {
                hits.push(self.decode(k.value(), &v.value())?);
            }
        }
        for pj in &hits {
            tbl.remove(pj.token.as_str()).map_err(err)?;
        }
        Ok(hits)
    }
    fn write_tx<R>(&self, f: impl FnOnce(&mut Table<'_>) -> Res<(bool, R)>) -> Res<R> {
        let tx = self.db.begin_write().map_err(err)?;
        let (commit, res) = f(&mut tx.open_table(TABLE).map_err(err)?)?;
        if commit {
            tx.commit().map_err(err)?;
        }
        Ok(res)
    }
    fn tx_or<R>(&self, fb: R, f: impl FnOnce(&mut Table<'_>) -> Res<(bool, R)>) -> R {
        or_log(self.write_tx(f), fb)
    }
}

impl Storage for RedbSessionStore {
    fn is_blocking(&self) -> bool {
        true
    }
    fn upsert(&self, pj: PendingJoin, now: Instant) -> Res<SessionToken> {
        let cap = u64::try_from(self.max_capacity).unwrap_or(u64::MAX);
        self.write_tx(|tbl| {
            self.take_where(tbl, |buf| is_peer(buf, pj.chat_id, pj.user_id))?;
            if tbl.len().map_err(err)? >= cap {
                let now_ms = self.to_ms(now);
                self.take_where(tbl, |buf| u64::from_le_bytes(le(buf, 36)) <= now_ms)?;
                if tbl.len().map_err(err)? >= cap {
                    return Err(StoreError::CapacityExceeded);
                }
            }
            tbl.insert(pj.token.as_str(), encode(&pj, self.to_ms(pj.expires_at)))
                .map_err(err)?;
            Ok((true, pj.token))
        })
    }
    fn set_message_id(&self, tok: &SessionToken, msg: MessageId) -> bool {
        self.tx_or(false, |tbl| {
            let Some(mut buf) = tbl.get(tok.as_str()).map_err(err)?.map(|g| g.value()) else {
                return Ok((false, false));
            };
            buf[31] = 1;
            buf[32..36].copy_from_slice(&msg.0.to_le_bytes());
            tbl.insert(tok.as_str(), buf).map_err(err)?;
            Ok((true, true))
        })
    }
    fn verify_attempt(
        &self,
        tok: &SessionToken,
        caller: UserId,
        ans: u8,
        now: Instant,
    ) -> AttemptOutcome {
        let (key, now_ms) = (tok.as_str(), self.to_ms(now));
        self.tx_or(NotFoundOrExpired, |tbl| {
            let Some(mut buf) = tbl.get(key).map_err(err)?.map(|g| g.value()) else {
                return Ok((false, NotFoundOrExpired));
            };
            if now_ms >= u64::from_le_bytes(le(&buf, 36)) {
                tbl.remove(key).map_err(err)?;
                return Ok((true, NotFoundOrExpired));
            }
            let mut pj = self.decode(key, &buf)?;
            if pj.user_id != caller {
                return Ok((false, WrongUser));
            }
            if pj.challenge.is_correct(ans) {
                tbl.remove(key).map_err(err)?;
                return Ok((true, Approved(pj)));
            }
            pj.attempts = pj.attempts.saturating_add(1);
            if pj.attempts < pj.max_attempts {
                buf[29] = pj.attempts;
                tbl.insert(key, buf).map_err(err)?;
                let remaining = pj.max_attempts - pj.attempts;
                return Ok((true, Retry { remaining }));
            }
            tbl.remove(key).map_err(err)?;
            Ok((true, DeclinedExhausted(pj)))
        })
    }
    fn remove_by_token(&self, tok: &SessionToken) -> Option<PendingJoin> {
        self.tx_or(None, |tbl| match tbl.remove(tok.as_str()).map_err(err)? {
            Some(g) => Ok((true, Some(self.decode(tok.as_str(), &g.value())?))),
            None => Ok((false, None)),
        })
    }
    fn remove_by_peer(&self, chat: ChatId, user: UserId) -> Option<PendingJoin> {
        self.tx_or(None, |tbl| {
            let res = self.take_where(tbl, |buf| is_peer(buf, chat, user))?.pop();
            Ok((res.is_some(), res))
        })
    }
    fn reap_expired(&self, now: Instant) -> Vec<PendingJoin> {
        let now_ms = self.to_ms(now);
        self.tx_or(Vec::new(), |tbl| {
            let out = self.take_where(tbl, |buf| u64::from_le_bytes(le(buf, 36)) <= now_ms)?;
            Ok((!out.is_empty(), out))
        })
    }
    fn len(&self) -> usize {
        let tx = self.db.begin_read().map_err(err);
        let n = tx.and_then(|t| t.open_table(TABLE).map_err(err)?.len().map_err(err));
        or_log(n.map(|v| usize::try_from(v).unwrap_or(usize::MAX)), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Tmp(std::path::PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn sess(chat: i64, user: u64, expires_at: Instant) -> PendingJoin {
        PendingJoin {
            token: SessionToken::generate(),
            chat_id: ChatId(chat),
            user_id: UserId(user),
            user_chat_id: ChatId(1),
            challenge: Challenge::generate(),
            attempts: 0,
            max_attempts: 2,
            message_id: None,
            expires_at,
        }
    }
    #[tokio::test]
    async fn redb_crud_capacity_attempts_reap_and_reopen() -> Res<()> {
        let dir = std::env::temp_dir().join(format!("botifil-{}", std::process::id()));
        let (_tmp, p) = (Tmp(dir.clone()), dir.join("s.redb"));
        let (max, now, sec) = (u64::MAX, Instant::now(), Duration::from_secs(60));
        let (st, past, exp) = (RedbSessionStore::open(&p, 2)?, now - sec, now + sec);
        assert!(crate::store::SessionStore::open_redb(p.join("x"), 1).is_err());
        let up = |c, u, e| st.upsert(sess(c, u, e), now);
        let (t1, t2) = (up(-100, 1, exp)?, up(-100, 1, exp)?);
        assert!(t1 != t2 && st.len() == 1 && st.remove_by_token(&t1).is_none());
        assert!(st.set_message_id(&t2, MessageId(-42)) && !st.set_message_id(&t1, MessageId(1)));
        let p1 = st.remove_by_peer(ChatId(-100), UserId(1));
        assert!(p1.is_some_and(|s| s.message_id == Some(MessageId(-42))) && st.is_empty());
        let (s2, s_live) = (sess(-200, 2, exp), sess(-300, max, exp));
        let (a2, t2) = (s2.challenge.correct_answer(), st.upsert(s2, now)?);
        let v = |t: &SessionToken, u, a| st.verify_attempt(t, UserId(u), a, now);
        assert!(v(&t2, 99, a2) == WrongUser && v(&t2, 2, a2 + 1) == Retry { remaining: 1 });
        assert!(matches!(v(&t2, 2, a2 + 1), DeclinedExhausted(s) if s.attempts == 2));
        let (al, live) = (s_live.challenge.correct_answer(), st.upsert(s_live, now)?);
        let dead = up(-400, 4, past)?;
        assert!(st.reap_expired(now).first().map(|s| &s.token) == Some(&dead) && st.len() == 1);
        let fill = up(-400, 4, exp)?;
        assert_eq!(up(-500, 5, exp), Err(StoreError::CapacityExceeded));
        assert!(st.remove_by_token(&fill).is_some());
        assert_eq!(v(&up(-700, 7, past)?, 7, 1), NotFoundOrExpired);
        assert!(up(-600, 6, past).is_ok() && up(-500, 5, exp).is_ok());
        assert!(st.set_message_id(&live, MessageId(55)));
        assert_eq!(v(&live, max, al + 1), Retry { remaining: 1 });
        drop(st);
        let r2 = crate::store::SessionStore::open_redb(&p, 2)?;
        assert_eq!(r2.len().await, 2);
        let a = r2.verify_attempt(&live, UserId(max), al, now).await;
        assert!(matches!(a, Approved(o) if o.attempts == 1 && o.message_id == Some(MessageId(55))));
        drop(r2);
        let r3 = crate::store::SessionStore::open_redb(&p, 2)?;
        assert_eq!(r3.len().await, 1);
        assert!(r3.remove_by_peer(ChatId(-500), UserId(5)).await.is_some() && r3.is_empty().await);
        Ok(())
    }
}
