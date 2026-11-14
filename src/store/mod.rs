mod memory;

use teloxide::types::{ChatId, MessageId, UserId};
use tokio::time::Instant;

use crate::domain::{PendingJoin, SessionToken};

pub use memory::{AttemptOutcome, DEFAULT_MAX_CAPACITY, MemoryStore, SessionStore, StoreError};

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
