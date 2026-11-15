use super::super::tests::session as sess;
use super::{AttemptOutcome::*, *};
use std::time::Duration;

fn session(user: u64, expires_at: Instant) -> PendingJoin {
    sess(-1, user, expires_at)
}

#[tokio::test]
async fn test_store() -> Result<(), StoreError> {
    let (store, now) = (SessionStore::new(2), Instant::now());
    let (past, exp) = (now - Duration::from_secs(10), now + Duration::from_secs(60));
    let t1 = store.upsert(session(1, exp), now).await?;
    let t2 = store.upsert(session(1, exp), now).await?;
    assert!(t1 != t2 && store.len().await == 1 && store.remove_by_token(&t1).await.is_none());
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
    let g1 = store.get_by_token(&t, now).await?;
    let g2 = store.get_by_token(&t, now).await?;
    assert!(g1.is_some_and(|p| p.attempts == 1) && g2.is_some_and(|p| p.attempts == 1));
    s.attempts = 2;
    let res = store.verify_attempt(&t, u2, ans + 1, now).await;
    assert_eq!(res, DeclinedExhausted(s));
    let s3 = session(3, exp);
    let (ans3, tok_ok) = (s3.challenge.correct_answer(), store.upsert(s3, now).await?);
    let res = store.verify_attempt(&tok_ok, UserId(3), ans3, now).await;
    assert!(matches!(res, Approved(_)));
    let tok_exp = store.upsert(session(4, past), past).await?;
    assert!(store.get_by_token(&tok_exp, now).await?.is_none() && store.is_empty().await);
    assert!(store.remove_by_peer(ChatId(-1), UserId(4)).await.is_none());
    let tok_exp2 = store.upsert(session(4, past), past).await?;
    let res1 = store.verify_attempt(&tok_exp2, UserId(4), ans, now).await;
    let dummy = SessionToken::generate();
    assert!(store.get_by_token(&dummy, now).await?.is_none());
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
    let e0 = MemoryStore::new(0).upsert(session(9, exp), now);
    assert_eq!(e0, Err(StoreError::CapacityExceeded));
    let s9 = SessionStore::from_storage(Arc::new(MemoryStore::default()));
    assert!(s9.upsert(session(9, exp), now).await.is_ok() && s9.len().await == 1);
    Ok(())
}
