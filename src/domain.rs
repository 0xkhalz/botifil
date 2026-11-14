use std::hash::{BuildHasher, RandomState};
use std::sync::atomic::{AtomicU64, Ordering};
use teloxide::types::{ChatId, MessageId, UserId};

pub const MAX_CALLBACK_DATA_BYTES: usize = 64;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid session token")]
    InvalidToken,
    #[error("invalid challenge")]
    InvalidChallenge,
    #[error("callback payload too long")]
    CallbackTooLong,
    #[error("invalid callback format")]
    InvalidCallbackFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionToken(String);

static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_rand() -> u64 {
    RandomState::new().hash_one(TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed))
}
fn rand_u8(bound: u64, offset: u8) -> u8 {
    u8::try_from((next_rand() % bound) + u64::from(offset)).unwrap_or(offset)
}

impl SessionToken {
    /// # Errors
    /// Returns [`DomainError::InvalidToken`] if raw is not 1..=32 ASCII alphanumeric chars.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let s = raw.into();
        let ok = !s.is_empty() && s.len() <= 32 && s.chars().all(|c| c.is_ascii_alphanumeric());
        ok.then_some(Self(s)).ok_or(DomainError::InvalidToken)
    }
    pub fn generate() -> Self {
        Self(format!("{:016x}", next_rand()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    pub left: u8,
    pub right: u8,
    pub choices: [u8; 3],
}

impl Challenge {
    const fn raw(left: u8, right: u8, choices: [u8; 3]) -> Self {
        Self {
            left,
            right,
            choices,
        }
    }
    /// # Errors
    /// Returns [`DomainError::InvalidChallenge`] if operands or choices are invalid.
    pub fn new(left: u8, right: u8, choices: [u8; 3]) -> Result<Self, DomainError> {
        let [a, b, c] = choices;
        let sum = left + right;
        let ok = (1..=9).contains(&left) && (1..=9).contains(&right);
        let valid = ok && choices.contains(&sum) && a != b && b != c && a != c;
        valid
            .then_some(Self::raw(left, right, choices))
            .ok_or(DomainError::InvalidChallenge)
    }
    pub fn generate() -> Self {
        let (left, right) = (rand_u8(9, 1), rand_u8(9, 1));
        let sum = left + right;
        let (d1, d2) = (rand_u8(16, 2), rand_u8(15, 2));
        let d1 = d1 + u8::from(d1 >= sum);
        let d2 = d2 + u8::from(d2 >= sum.min(d1)) + u8::from(d2 >= sum.max(d1) - 1);
        let mut choices = [sum, d1, d2];
        choices.swap(0, usize::from(rand_u8(3, 0)));
        choices.swap(1, usize::from(rand_u8(2, 1)));
        Self::raw(left, right, choices)
    }
    pub fn correct_answer(&self) -> u8 {
        self.left + self.right
    }
    pub fn is_correct(&self, answer: u8) -> bool {
        answer == self.correct_answer()
    }
    pub fn prompt_text(&self) -> String {
        let (l, r) = (self.left, self.right);
        format!("Verify you are human: what is {l} + {r}?")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackAction {
    pub token: SessionToken,
    pub answer: u8,
}

impl CallbackAction {
    pub const fn new(token: SessionToken, answer: u8) -> Self {
        Self { token, answer }
    }
    /// # Errors
    /// Returns [`DomainError::CallbackTooLong`] if payload exceeds 64 bytes.
    pub fn encode(&self) -> Result<String, DomainError> {
        let raw = format!("v1:{}:{}", self.token.0, self.answer);
        (raw.len() <= MAX_CALLBACK_DATA_BYTES)
            .then_some(raw)
            .ok_or(DomainError::CallbackTooLong)
    }
    /// # Errors
    /// Returns [`DomainError`] if payload is malformed or invalid.
    pub fn decode(raw: &str) -> Result<Self, DomainError> {
        let mut p = raw.split(':');
        match (p.next(), p.next(), p.next(), p.next()) {
            (Some("v1"), Some(t), Some(a), None) if raw.len() <= MAX_CALLBACK_DATA_BYTES => {
                let token = SessionToken::new(t)?;
                let answer = a.parse().map_err(|_| DomainError::InvalidCallbackFormat)?;
                Ok(Self::new(token, answer))
            }
            _ if raw.len() > MAX_CALLBACK_DATA_BYTES => Err(DomainError::CallbackTooLong),
            _ => Err(DomainError::InvalidCallbackFormat),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingJoin {
    pub token: SessionToken,
    pub chat_id: ChatId,
    pub user_id: UserId,
    pub user_chat_id: ChatId,
    pub challenge: Challenge,
    pub attempts: u8,
    pub max_attempts: u8,
    pub message_id: Option<MessageId>,
    pub expires_at: tokio::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionOutcome {
    ChallengeSent,
    Approved,
    RetryAllowed { remaining: u8 },
    DeclinedExhausted,
    DeclinedScreened,
    LeftPendingBlocked,
    LeftPendingExpired,
    IdempotentNoOp,
    IdempotentAlreadyMember,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain() -> Result<(), DomainError> {
        let (ch, tok) = (Challenge::generate(), SessionToken::generate());
        assert!(ch.is_correct(ch.correct_answer()) && Challenge::new(0, 5, [5, 6, 7]).is_err());
        assert_eq!(ch, Challenge::new(ch.left, ch.right, ch.choices)?);
        let a = CallbackAction::new(tok, 7);
        let enc = a.encode()?;
        assert!(enc.len() <= MAX_CALLBACK_DATA_BYTES && a == CallbackAction::decode(&enc)?);
        for bad in ["", "v2:t:1", "v1:t:x", &format!("v1:{}:1", "a".repeat(65))] {
            assert!(CallbackAction::decode(bad).is_err());
        }
        let long = CallbackAction::new(SessionToken("x".repeat(65)), 1);
        assert_eq!(long.encode(), Err(DomainError::CallbackTooLong));
        Ok(())
    }
}
