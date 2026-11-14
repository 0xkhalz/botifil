use std::{collections::HashMap, sync::Arc};
use teloxide::types::{ChatId, MessageId, UserId};
use tokio::{sync::RwLock, time::Instant};

use crate::domain::{PendingJoin, SessionToken};

pub const DEFAULT_MAX_CAPACITY: usize = 4096;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StoreError {
    #[error("session store capacity exceeded")]
    CapacityExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptOutcome {
    Approved(PendingJoin),
    Retry { remaining: u8 },
    DeclinedExhausted(PendingJoin),
    WrongUser,
    NotFoundOrExpired,
}

impl AttemptOutcome {
    const fn retry(remaining: u8) -> Self {
        Self::Retry { remaining }
    }

    fn from_session(s: Option<PendingJoin>, f: fn(PendingJoin) -> Self) -> Self {
        s.map_or(Self::NotFoundOrExpired, f)
    }
}

#[derive(Default)]
struct StoreInner {
    max_capacity: usize,
    by_token: HashMap<SessionToken, PendingJoin>,
    by_peer: HashMap<(ChatId, UserId), SessionToken>,
}

impl StoreInner {
    fn remove_token(&mut self, token: &SessionToken) -> Option<PendingJoin> {
        let s = self.by_token.remove(token)?;
        let peer = (s.chat_id, s.user_id);
        if self.by_peer.get(&peer) == Some(token) {
            self.by_peer.remove(&peer);
        }
        Some(s)
    }

    fn reap_expired(&mut self, now: Instant) -> Vec<PendingJoin> {
        let expired = |s: &PendingJoin| (now >= s.expires_at).then(|| s.token.clone());
        let exp: Vec<_> = self.by_token.values().filter_map(expired).collect();
        exp.iter().filter_map(|t| self.remove_token(t)).collect()
    }
}

#[derive(Clone)]
pub struct SessionStore(Arc<RwLock<StoreInner>>);

impl Default for SessionStore {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_CAPACITY)
    }
}

impl SessionStore {
    #[must_use]
    pub fn new(max_capacity: usize) -> Self {
        Self(Arc::new(RwLock::new(StoreInner {
            max_capacity,
            ..Default::default()
        })))
    }

    /// Inserts or replaces a pending join session.
    ///
    /// # Errors
    /// Returns [`StoreError::CapacityExceeded`] if capacity is full and no expired sessions exist.
    pub async fn upsert(&self, s: PendingJoin, now: Instant) -> Result<SessionToken, StoreError> {
        let mut inner = self.0.write().await;
        let peer = (s.chat_id, s.user_id);
        if let Some(old) = inner.by_peer.remove(&peer) {
            inner.by_token.remove(&old);
        }
        if inner.by_token.len() >= inner.max_capacity {
            inner.reap_expired(now);
            if inner.by_token.len() >= inner.max_capacity {
                return Err(StoreError::CapacityExceeded);
            }
        }
        let tok = s.token.clone();
        inner.by_peer.insert(peer, tok.clone());
        inner.by_token.insert(tok.clone(), s);
        Ok(tok)
    }

    pub async fn set_message_id(&self, tok: &SessionToken, msg: MessageId) -> bool {
        let mut inner = self.0.write().await;
        inner.by_token.get_mut(tok).is_some_and(|s| {
            s.message_id = Some(msg);
            true
        })
    }

    pub async fn verify_attempt(
        &self,
        tok: &SessionToken,
        caller: UserId,
        ans: u8,
        now: Instant,
    ) -> AttemptOutcome {
        let mut inner = self.0.write().await;
        let Some(s) = inner.by_token.get(tok) else {
            return AttemptOutcome::NotFoundOrExpired;
        };
        if now >= s.expires_at {
            inner.remove_token(tok);
            return AttemptOutcome::NotFoundOrExpired;
        }
        if s.user_id != caller {
            return AttemptOutcome::WrongUser;
        }
        if s.challenge.is_correct(ans) {
            return AttemptOutcome::from_session(inner.remove_token(tok), AttemptOutcome::Approved);
        }
        let Some(s) = inner.by_token.get_mut(tok) else {
            return AttemptOutcome::NotFoundOrExpired;
        };
        s.attempts = s.attempts.saturating_add(1);
        if s.attempts < s.max_attempts {
            return AttemptOutcome::retry(s.max_attempts - s.attempts);
        }
        AttemptOutcome::from_session(inner.remove_token(tok), AttemptOutcome::DeclinedExhausted)
    }

    pub async fn remove_by_token(&self, tok: &SessionToken) -> Option<PendingJoin> {
        self.0.write().await.remove_token(tok)
    }

    pub async fn remove_by_peer(&self, chat: ChatId, user: UserId) -> Option<PendingJoin> {
        let mut inner = self.0.write().await;
        let t = inner.by_peer.remove(&(chat, user))?;
        inner.by_token.remove(&t)
    }

    pub async fn reap_expired(&self, now: Instant) -> Vec<PendingJoin> {
        self.0.write().await.reap_expired(now)
    }

    pub async fn len(&self) -> usize {
        self.0.read().await.by_token.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.0.read().await.by_token.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{AttemptOutcome::*, *};
    use crate::domain::Challenge;
    use std::time::Duration;

    fn session(user: u64, expires_at: Instant) -> PendingJoin {
        PendingJoin {
            token: SessionToken::generate(),
            chat_id: ChatId(-1),
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
    async fn test_store() -> Result<(), StoreError> {
        let store = SessionStore::new(2);
        let now = Instant::now();
        let (past, exp) = (now - Duration::from_secs(10), now + Duration::from_secs(60));

        let t1 = store.upsert(session(1, exp), now).await?;
        let t2 = store.upsert(session(1, exp), now).await?;
        assert_ne!(t1, t2);
        assert_eq!(store.len().await, 1);
        assert!(store.remove_by_token(&t1).await.is_none());
        assert!(store.set_message_id(&t2, MessageId(10)).await);
        assert!(store.remove_by_peer(ChatId(-1), UserId(1)).await.is_some());
        assert!(store.is_empty().await);

        let mut s = session(2, exp);
        let t = store.upsert(s.clone(), now).await?;
        let (ans, u2) = (s.challenge.correct_answer(), UserId(2));
        let res = store.verify_attempt(&t, UserId(99), ans, now).await;
        assert_eq!(res, WrongUser);
        let res = store.verify_attempt(&t, u2, ans + 1, now).await;
        assert_eq!(res, AttemptOutcome::retry(1));
        s.attempts = 2;
        let res = store.verify_attempt(&t, u2, ans + 1, now).await;
        assert_eq!(res, DeclinedExhausted(s));
        let s3 = session(3, exp);
        let (ans3, tok_ok) = (s3.challenge.correct_answer(), store.upsert(s3, now).await?);
        let res = store.verify_attempt(&tok_ok, UserId(3), ans3, now).await;
        assert!(matches!(res, Approved(_)));
        let tok_exp = store.upsert(session(4, past), past).await?;
        let res1 = store.verify_attempt(&tok_exp, UserId(4), ans, now).await;
        let dummy = SessionToken::generate();
        let res2 = store.verify_attempt(&dummy, UserId(1), ans, now).await;
        assert!(res1 == NotFoundOrExpired && res2 == NotFoundOrExpired);
        let t5 = store.upsert(session(5, past), past).await?;
        let t6 = store.upsert(session(6, exp), past).await?;
        assert_eq!(store.len().await, 2);
        let reaped = store.reap_expired(now).await;
        assert_eq!(reaped.first().map(|s| &s.token), Some(&t5));
        assert_eq!(store.len().await, 1);
        store.upsert(session(7, past), now).await?;
        let t8 = store.upsert(session(8, exp), now).await?;
        assert_eq!(store.len().await, 2);
        let err = store.upsert(session(9, exp), now).await;
        assert_eq!(err, Err(StoreError::CapacityExceeded));
        assert!(store.remove_by_token(&t6).await.is_some());
        assert!(store.remove_by_token(&t8).await.is_some() && store.is_empty().await);
        Ok(())
    }
}
