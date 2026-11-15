mod memory;
mod redb;

use std::path::Path;
use teloxide::types::{ChatId, MessageId, UserId};
use tokio::time::Instant;

use crate::domain::{PendingJoin, SessionToken};

pub use memory::{AttemptOutcome, DEFAULT_MAX_CAPACITY, MemoryStore, SessionStore, StoreError};
pub use redb::RedbSessionStore;

impl SessionStore {
    /// Opens a persistent `redb` session store at `path`.
    ///
    /// # Errors
    /// Returns [`StoreError::Backend`] if opening or initializing the database fails.
    pub fn open_redb(path: impl AsRef<Path>, max_capacity: usize) -> Result<Self, StoreError> {
        let store = RedbSessionStore::open(path, max_capacity)?;
        Ok(Self::from_storage(std::sync::Arc::new(store)))
    }
}

pub trait Storage: Send + Sync {
    fn is_blocking(&self) -> bool {
        false
    }
    /// # Errors
    /// Returns [`StoreError`] on capacity limit or backend failure.
    fn upsert(&self, s: PendingJoin, now: Instant) -> Result<SessionToken, StoreError>;
    fn set_message_id(&self, tok: &SessionToken, msg: MessageId) -> bool;
    fn get_by_token(&self, tok: &SessionToken, now: Instant) -> Option<PendingJoin>;
    fn verify_attempt(
        &self,
        tok: &SessionToken,
        caller: UserId,
        ans: u8,
        now: Instant,
    ) -> AttemptOutcome;
    fn remove_by_token(&self, tok: &SessionToken) -> Option<PendingJoin>;
    fn remove_by_peer(&self, chat: ChatId, user: UserId) -> Option<PendingJoin>;
    fn reap_expired(&self, now: Instant) -> Vec<PendingJoin>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::domain::Challenge;
    use AttemptOutcome::{Approved, DeclinedExhausted, NotFoundOrExpired, Retry, WrongUser};
    use std::{sync::Arc, time::Duration};

    pub(super) fn session(chat: i64, user: u64, expires_at: Instant) -> PendingJoin {
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
    struct Blocking(Option<MemoryStore>);
    impl Blocking {
        fn inner(&self) -> &MemoryStore {
            self.0
                .as_ref()
                .unwrap_or_else(|| std::panic::resume_unwind(Box::new("boom")))
        }
    }
    impl Storage for Blocking {
        fn is_blocking(&self) -> bool {
            true
        }
        fn upsert(&self, s: PendingJoin, now: Instant) -> Result<SessionToken, StoreError> {
            self.inner().upsert(s, now)
        }
        fn set_message_id(&self, tok: &SessionToken, msg: MessageId) -> bool {
            self.inner().set_message_id(tok, msg)
        }
        fn get_by_token(&self, tok: &SessionToken, now: Instant) -> Option<PendingJoin> {
            self.inner().get_by_token(tok, now)
        }
        fn verify_attempt(
            &self,
            tok: &SessionToken,
            caller: UserId,
            ans: u8,
            now: Instant,
        ) -> AttemptOutcome {
            self.inner().verify_attempt(tok, caller, ans, now)
        }
        fn remove_by_token(&self, tok: &SessionToken) -> Option<PendingJoin> {
            self.inner().remove_by_token(tok)
        }
        fn remove_by_peer(&self, chat: ChatId, user: UserId) -> Option<PendingJoin> {
            self.inner().remove_by_peer(chat, user)
        }
        fn reap_expired(&self, now: Instant) -> Vec<PendingJoin> {
            self.inner().reap_expired(now)
        }
        fn len(&self) -> usize {
            self.inner().len()
        }
    }

    #[tokio::test]
    async fn blocking_dispatch_and_join_error_fallback() -> Result<(), StoreError> {
        let now = Instant::now();
        let join = session(-1, 7, now + Duration::from_secs(60));
        let ans = join.challenge.correct_answer();
        let st = SessionStore::from_storage(Arc::new(Blocking(Some(MemoryStore::new(2)))));
        let tok = st.upsert(join.clone(), now).await?;
        assert!(st.set_message_id(&tok, MessageId(42)).await && st.len().await == 1);
        assert!(
            st.get_by_token(&tok, now)
                .await
                .is_some_and(|s| s.attempts == 0 && s.message_id == Some(MessageId(42)))
        );
        let ok = st.verify_attempt(&tok, UserId(7), ans, now).await;
        assert!(matches!(ok, Approved(ref s) if s.message_id == Some(MessageId(42))));
        let t2 = st.upsert(join.clone(), now).await?;
        assert!(st.remove_by_token(&t2).await.is_some());
        st.upsert(join.clone(), now).await?;
        assert!(st.remove_by_peer(ChatId(-1), UserId(7)).await.is_some());
        assert!(st.is_empty().await && st.reap_expired(now).await.is_empty());
        let bad = SessionStore::from_storage(Arc::new(Blocking(None)));
        let err = StoreError::Backend("blocking storage task failed".into());
        assert!(err.to_string().starts_with("storage backend error: "));
        assert_eq!(bad.upsert(join, now).await, Err(err));
        assert!(!bad.set_message_id(&tok, MessageId(1)).await);
        assert!(bad.get_by_token(&tok, now).await.is_none());
        let out = bad.verify_attempt(&tok, UserId(7), ans, now).await;
        assert_eq!(out, NotFoundOrExpired);
        assert!(bad.remove_by_token(&tok).await.is_none());
        assert!(bad.remove_by_peer(ChatId(-1), UserId(7)).await.is_none());
        assert!(bad.reap_expired(now).await.is_empty() && bad.is_empty().await);
        assert_eq!(bad.len().await, 0);
        Ok(())
    }

    #[tokio::test]
    async fn memory_store_crud_and_get_by_token() -> Result<(), StoreError> {
        let (store, now) = (SessionStore::new(2), Instant::now());
        let (past, exp) = (now - Duration::from_secs(10), now + Duration::from_secs(60));
        let (t1, t2) = (
            store.upsert(session(-1, 1, exp), now).await?,
            store.upsert(session(-1, 1, exp), now).await?,
        );
        assert!(t1 != t2 && store.len().await == 1 && store.remove_by_token(&t1).await.is_none());
        assert!(store.set_message_id(&t2, MessageId(10)).await);
        assert!(
            store.remove_by_peer(ChatId(-1), UserId(1)).await.is_some() && store.is_empty().await
        );
        let mut s = session(-1, 2, exp);
        let t = store.upsert(s.clone(), now).await?;
        let (ans, u2) = (s.challenge.correct_answer(), UserId(2));
        assert_eq!(
            store.verify_attempt(&t, UserId(99), ans, now).await,
            WrongUser
        );
        assert_eq!(
            store.verify_attempt(&t, u2, ans + 1, now).await,
            Retry { remaining: 1 }
        );
        assert!(
            store
                .get_by_token(&t, now)
                .await
                .is_some_and(|p| p.attempts == 1)
        );
        assert!(
            store
                .get_by_token(&t, now)
                .await
                .is_some_and(|p| p.attempts == 1)
        );
        s.attempts = 2;
        assert_eq!(
            store.verify_attempt(&t, u2, ans + 1, now).await,
            DeclinedExhausted(s)
        );
        let s3 = session(-1, 3, exp);
        let (ans3, tok_ok) = (s3.challenge.correct_answer(), store.upsert(s3, now).await?);
        assert!(matches!(
            store.verify_attempt(&tok_ok, UserId(3), ans3, now).await,
            Approved(_)
        ));
        let tok_exp = store.upsert(session(-1, 4, past), past).await?;
        assert!(store.get_by_token(&tok_exp, now).await.is_none() && store.is_empty().await);
        assert!(store.remove_by_peer(ChatId(-1), UserId(4)).await.is_none());
        let tok_exp2 = store.upsert(session(-1, 4, past), past).await?;
        assert_eq!(
            store.verify_attempt(&tok_exp2, UserId(4), ans, now).await,
            NotFoundOrExpired
        );
        assert_eq!(
            store
                .verify_attempt(&SessionToken::generate(), UserId(1), ans, now)
                .await,
            NotFoundOrExpired
        );
        let t5 = store.upsert(session(-1, 5, past), past).await?;
        let t6 = store.upsert(session(-1, 6, exp), past).await?;
        assert_eq!(
            store.reap_expired(now).await.first().map(|s| &s.token),
            Some(&t5)
        );
        store.upsert(session(-1, 7, past), now).await?;
        let t8 = store.upsert(session(-1, 8, exp), now).await?;
        assert_eq!(
            store.upsert(session(-1, 9, exp), now).await,
            Err(StoreError::CapacityExceeded)
        );
        assert!(
            store.remove_by_token(&t6).await.is_some()
                && store.remove_by_token(&t8).await.is_some()
        );
        assert_eq!(
            MemoryStore::new(0).upsert(session(-1, 9, exp), now),
            Err(StoreError::CapacityExceeded)
        );
        let s9 = SessionStore::from_storage(Arc::new(MemoryStore::default()));
        assert!(s9.upsert(session(-1, 9, exp), now).await.is_ok() && s9.len().await == 1);
        Ok(())
    }
}
