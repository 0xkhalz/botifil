use std::{
    collections::HashMap,
    sync::{Arc, PoisonError, RwLock, RwLockWriteGuard},
};
use teloxide::types::{ChatId, MessageId, UserId};
use tokio::time::Instant;

use super::Storage;
use crate::domain::{PendingJoin, SessionToken};

pub const DEFAULT_MAX_CAPACITY: usize = 4096;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StoreError {
    #[error("session store capacity exceeded")]
    CapacityExceeded,
    #[error("storage backend error: {0}")]
    Backend(String),
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

pub struct MemoryStore(RwLock<StoreInner>);

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_CAPACITY)
    }
}

impl MemoryStore {
    #[must_use]
    pub fn new(max_capacity: usize) -> Self {
        Self(RwLock::new(StoreInner {
            max_capacity,
            ..Default::default()
        }))
    }
    fn write(&self) -> RwLockWriteGuard<'_, StoreInner> {
        self.0.write().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Storage for MemoryStore {
    fn upsert(&self, s: PendingJoin, now: Instant) -> Result<SessionToken, StoreError> {
        let mut inner = self.write();
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
    fn set_message_id(&self, tok: &SessionToken, msg: MessageId) -> bool {
        self.write().by_token.get_mut(tok).is_some_and(|s| {
            s.message_id = Some(msg);
            true
        })
    }
    fn get_by_token(&self, tok: &SessionToken, now: Instant) -> Option<PendingJoin> {
        let mut inner = self.write();
        let s = inner.by_token.get(tok)?;
        if now >= s.expires_at {
            inner.remove_token(tok);
            return None;
        }
        Some(s.clone())
    }
    fn verify_attempt(
        &self,
        tok: &SessionToken,
        caller: UserId,
        ans: u8,
        now: Instant,
    ) -> AttemptOutcome {
        use AttemptOutcome::{Approved, DeclinedExhausted, NotFoundOrExpired, WrongUser};
        let mut inner = self.write();
        let Some(s) = inner.by_token.get_mut(tok) else {
            return NotFoundOrExpired;
        };
        if now >= s.expires_at {
            inner.remove_token(tok);
            return NotFoundOrExpired;
        }
        if s.user_id != caller {
            return WrongUser;
        }
        if s.challenge.is_correct(ans) {
            return AttemptOutcome::from_session(inner.remove_token(tok), Approved);
        }
        s.attempts = s.attempts.saturating_add(1);
        if s.attempts < s.max_attempts {
            return AttemptOutcome::retry(s.max_attempts - s.attempts);
        }
        AttemptOutcome::from_session(inner.remove_token(tok), DeclinedExhausted)
    }
    fn remove_by_token(&self, tok: &SessionToken) -> Option<PendingJoin> {
        self.write().remove_token(tok)
    }
    fn remove_by_peer(&self, chat: ChatId, user: UserId) -> Option<PendingJoin> {
        let mut inner = self.write();
        let t = inner.by_peer.remove(&(chat, user))?;
        inner.by_token.remove(&t)
    }
    fn reap_expired(&self, now: Instant) -> Vec<PendingJoin> {
        self.write().reap_expired(now)
    }
    fn len(&self) -> usize {
        let guard = self.0.read().unwrap_or_else(PoisonError::into_inner);
        guard.by_token.len()
    }
}

#[derive(Clone)]
pub struct SessionStore(Arc<dyn Storage>);

impl Default for SessionStore {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_CAPACITY)
    }
}

impl SessionStore {
    #[must_use]
    pub fn new(max_capacity: usize) -> Self {
        Self(Arc::new(MemoryStore::new(max_capacity)))
    }
    #[must_use]
    pub fn from_storage(storage: Arc<dyn Storage>) -> Self {
        Self(storage)
    }
    async fn exec<R: Send + 'static>(
        &self,
        fb: R,
        f: impl FnOnce(&dyn Storage) -> R + Send + 'static,
    ) -> R {
        if !self.0.is_blocking() {
            return f(&*self.0);
        }
        let st = Arc::clone(&self.0);
        tokio::task::spawn_blocking(move || f(&*st))
            .await
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "blocking storage task failed");
                fb
            })
    }
    /// # Errors
    /// Returns [`StoreError`] if capacity is full or backend persistence fails.
    pub async fn upsert(&self, s: PendingJoin, now: Instant) -> Result<SessionToken, StoreError> {
        let fb = Err(StoreError::Backend("blocking storage task failed".into()));
        self.exec(fb, move |st| st.upsert(s, now)).await
    }
    pub async fn set_message_id(&self, tok: &SessionToken, msg: MessageId) -> bool {
        let t = tok.clone();
        self.exec(false, move |st| st.set_message_id(&t, msg)).await
    }
    pub async fn get_by_token(&self, tok: &SessionToken, now: Instant) -> Option<PendingJoin> {
        let t = tok.clone();
        self.exec(None, move |st| st.get_by_token(&t, now)).await
    }
    pub async fn verify_attempt(
        &self,
        tok: &SessionToken,
        caller: UserId,
        ans: u8,
        now: Instant,
    ) -> AttemptOutcome {
        let t = tok.clone();
        self.exec(AttemptOutcome::NotFoundOrExpired, move |st| {
            st.verify_attempt(&t, caller, ans, now)
        })
        .await
    }
    pub async fn remove_by_token(&self, tok: &SessionToken) -> Option<PendingJoin> {
        let t = tok.clone();
        self.exec(None, move |st| st.remove_by_token(&t)).await
    }
    pub async fn remove_by_peer(&self, chat: ChatId, user: UserId) -> Option<PendingJoin> {
        self.exec(None, move |st| st.remove_by_peer(chat, user))
            .await
    }
    pub async fn reap_expired(&self, now: Instant) -> Vec<PendingJoin> {
        self.exec(Vec::new(), move |st| st.reap_expired(now)).await
    }
    pub async fn len(&self) -> usize {
        self.exec(0, |st| st.len()).await
    }
    pub async fn is_empty(&self) -> bool {
        self.exec(true, |st| st.is_empty()).await
    }
}
