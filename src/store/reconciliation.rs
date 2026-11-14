use tokio::time::Instant;

use super::SessionStore;

/// Summary counts produced by startup session reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconcileStats {
    /// Number of expired sessions evicted during startup reconciliation.
    pub evicted: usize,
    /// Number of unexpired active sessions remaining in the store.
    pub active: usize,
}

/// Evicts sessions that expired before `now` and logs non-PII reconciliation counts.
pub async fn reconcile_on_startup(store: &SessionStore, now: Instant) -> ReconcileStats {
    let evicted = store.reap_expired(now).await.len();
    let active = store.len().await;
    tracing::info!(evicted, active, "Completed startup session reconciliation");
    ReconcileStats { evicted, active }
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use super::*;
    use crate::domain::{Challenge, PendingJoin, SessionToken};
    use crate::store::{AttemptOutcome as A, StoreError};
    use std::time::Duration;
    use teloxide::types::{ChatId, UserId};

    fn pending(user: u64, expires_at: Instant) -> PendingJoin {
        PendingJoin { token: SessionToken::generate(), chat_id: ChatId(-100), user_id: UserId(user), user_chat_id: ChatId(i64::try_from(user).unwrap_or(1)), challenge: Challenge::generate(), attempts: 0, max_attempts: 2, message_id: None, expires_at }
    }

    #[tokio::test]
    #[rustfmt::skip]
    async fn reconcile_on_startup_evicts_expired_and_counts_active() -> Result<(), StoreError> {
        let (st, now, s) = (SessionStore::new(8), Instant::now(), Duration::from_secs(30));
        let exp1 = st.upsert(pending(1, now - s), now - s).await?;
        let exp2 = st.upsert(pending(2, now), now - s).await?;
        let live = pending(3, now + s);
        let (ans, tok) = (live.challenge.correct_answer(), st.upsert(live, now).await?);

        assert_eq!(reconcile_on_startup(&st, now).await, ReconcileStats { evicted: 2, active: 1 });
        assert_eq!(st.len().await, 1);
        assert_eq!(st.verify_attempt(&exp1, UserId(1), 0, now).await, A::NotFoundOrExpired);
        assert_eq!(st.verify_attempt(&exp2, UserId(2), 0, now).await, A::NotFoundOrExpired);
        assert!(matches!(st.verify_attempt(&tok, UserId(3), ans, now).await, A::Approved(_)));
        Ok(())
    }
}
