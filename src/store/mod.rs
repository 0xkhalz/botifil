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
mod tests {
    use super::*;
    use crate::domain::Challenge;
    use AttemptOutcome::{Approved, NotFoundOrExpired};
    use std::{sync::Arc, time::Duration};
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
        let join = PendingJoin {
            token: SessionToken::generate(),
            chat_id: ChatId(-1),
            user_id: UserId(7),
            user_chat_id: ChatId(7),
            challenge: Challenge::generate(),
            attempts: 0,
            max_attempts: 2,
            message_id: None,
            expires_at: now + Duration::from_secs(60),
        };
        let ans = join.challenge.correct_answer();
        let st = SessionStore::from_storage(Arc::new(Blocking(Some(MemoryStore::new(2)))));
        let tok = st.upsert(join.clone(), now).await?;
        assert!(st.set_message_id(&tok, MessageId(42)).await && st.len().await == 1);
        let ok = st.verify_attempt(&tok, UserId(7), ans, now).await;
        assert!(matches!(ok, Approved(ref s) if s.message_id == Some(MessageId(42))));
        assert!(
            st.remove_by_token(&st.upsert(join.clone(), now).await?)
                .await
                .is_some()
        );
        st.upsert(join.clone(), now).await?;
        assert!(st.remove_by_peer(ChatId(-1), UserId(7)).await.is_some());
        assert!(st.is_empty().await && st.reap_expired(now).await.is_empty());
        let bad = SessionStore::from_storage(Arc::new(Blocking(None)));
        let err = StoreError::Backend("blocking storage task failed".into());
        assert!(err.to_string().starts_with("storage backend error: "));
        assert_eq!(bad.upsert(join, now).await, Err(err));
        assert!(!bad.set_message_id(&tok, MessageId(1)).await);
        let out = bad.verify_attempt(&tok, UserId(7), ans, now).await;
        assert_eq!(out, NotFoundOrExpired);
        assert!(bad.remove_by_token(&tok).await.is_none());
        assert!(bad.remove_by_peer(ChatId(-1), UserId(7)).await.is_none());
        assert!(bad.reap_expired(now).await.is_empty() && bad.is_empty().await);
        assert_eq!(bad.len().await, 0);
        Ok(())
    }
}
