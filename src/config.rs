use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use teloxide::types::ChatId;

/// Default timeout for completing a join request challenge (in seconds).
pub const DEFAULT_CHALLENGE_TTL_SECS: u64 = 300;
/// Maximum allowed timeout for completing a challenge (in seconds).
pub const MAX_CHALLENGE_TTL_SECS: u64 = 300;
/// Default maximum challenge answer attempts before decline.
pub const DEFAULT_MAX_ATTEMPTS: u8 = 2;
/// Maximum allowed challenge answer attempts.
pub const MAX_ALLOWED_ATTEMPTS: u8 = 5;
/// Default filesystem path for the persistent `redb` session database.
pub const DEFAULT_DB_PATH: &str = "./data/botifil.redb";

/// Configuration errors for bot initialization.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("TELOXIDE_TOKEN environment variable is not set")]
    MissingToken,
    #[error("TELOXIDE_TOKEN environment variable cannot be empty or whitespace")]
    EmptyToken,
    #[error("BOTIFIL_CHALLENGE_TTL_SECS must be an integer between 1 and 300")]
    InvalidChallengeTtl,
    #[error("BOTIFIL_MAX_ATTEMPTS must be an integer between 1 and 5")]
    InvalidMaxAttempts,
    #[error("BOTIFIL_ALLOWED_CHATS must be a comma-separated list of integer chat IDs")]
    InvalidAllowedChats,
    #[error("BOTIFIL_DB_PATH cannot be empty or whitespace")]
    InvalidDbPath,
}

/// Runtime bot configuration holding the Telegram bot token and policy settings.
#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    telegram_token: String,
    challenge_ttl_secs: u64,
    max_attempts: u8,
    allowed_chats: Vec<ChatId>,
    db_path: PathBuf,
}

fn parse_bounded<T: std::str::FromStr + PartialOrd>(
    val: Option<&str>,
    (def, min, max): (T, T, T),
    err: ConfigError,
) -> Result<T, ConfigError> {
    match val.map(str::trim) {
        None | Some("") => Ok(def),
        Some(s) => s.parse().ok().filter(|v| *v >= min && *v <= max).ok_or(err),
    }
}

fn parse_chats(raw: Option<&str>) -> Result<Vec<ChatId>, ConfigError> {
    match raw.map(str::trim) {
        None | Some("") => Ok(Vec::new()),
        Some(s) => {
            let err = |_| ConfigError::InvalidAllowedChats;
            let mut chats = Vec::new();
            for p in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                chats.push(ChatId(p.parse().map_err(err)?));
            }
            (!chats.is_empty())
                .then_some(chats)
                .ok_or(ConfigError::InvalidAllowedChats)
        }
    }
}

fn parse_db_path(raw: Option<&str>) -> Result<PathBuf, ConfigError> {
    match raw.map(str::trim) {
        None => Ok(PathBuf::from(DEFAULT_DB_PATH)),
        Some("") => Err(ConfigError::InvalidDbPath),
        Some(path) => Ok(PathBuf::from(path)),
    }
}

impl Config {
    /// Loads configuration from environment variables and `.env` file.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if any environment variable has an invalid value.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();
        let v = |k| std::env::var(k).ok();
        Self::from_env_vars(
            v("TELOXIDE_TOKEN"),
            v("BOTIFIL_CHALLENGE_TTL_SECS").as_deref(),
            v("BOTIFIL_MAX_ATTEMPTS").as_deref(),
            v("BOTIFIL_ALLOWED_CHATS").as_deref(),
            v("BOTIFIL_DB_PATH").as_deref(),
        )
    }

    /// Creates a configuration from an optional token string with default policy.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::MissingToken`] if the token is `None`,
    /// or [`ConfigError::EmptyToken`] if it is empty or whitespace.
    pub fn from_token(token: Option<String>) -> Result<Self, ConfigError> {
        Self::from_env_vars(token, None, None, None, None)
    }

    /// Creates a configuration from raw environment variable values.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if any setting fails validation or parsing.
    pub fn from_env_vars(
        token: Option<String>,
        ttl: Option<&str>,
        max_attempts: Option<&str>,
        allowed_chats: Option<&str>,
        db_path: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let telegram_token = match token {
            None => return Err(ConfigError::MissingToken),
            Some(t) if t.trim().is_empty() => return Err(ConfigError::EmptyToken),
            Some(t) => t,
        };

        Ok(Self {
            telegram_token,
            challenge_ttl_secs: parse_bounded(
                ttl,
                (DEFAULT_CHALLENGE_TTL_SECS, 1, MAX_CHALLENGE_TTL_SECS),
                ConfigError::InvalidChallengeTtl,
            )?,
            max_attempts: parse_bounded(
                max_attempts,
                (DEFAULT_MAX_ATTEMPTS, 1, MAX_ALLOWED_ATTEMPTS),
                ConfigError::InvalidMaxAttempts,
            )?,
            allowed_chats: parse_chats(allowed_chats)?,
            db_path: parse_db_path(db_path)?,
        })
    }

    /// Returns the Telegram bot token.
    #[must_use]
    pub fn telegram_token(&self) -> &str {
        &self.telegram_token
    }

    /// Returns the challenge timeout duration.
    #[must_use]
    pub fn challenge_ttl(&self) -> Duration {
        Duration::from_secs(self.challenge_ttl_secs)
    }

    /// Returns the maximum challenge attempts allowed.
    #[must_use]
    pub fn max_attempts(&self) -> u8 {
        self.max_attempts
    }

    /// Returns the filesystem path to the persistent session database.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Checks if a chat ID is allowed by configuration.
    #[must_use]
    pub fn is_chat_allowed(&self, chat_id: ChatId) -> bool {
        self.allowed_chats.is_empty() || self.allowed_chats.contains(&chat_id)
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("telegram_token", &"[REDACTED]")
            .field("challenge_ttl_secs", &self.challenge_ttl_secs)
            .field("max_attempts", &self.max_attempts)
            .field("allowed_chats_count", &self.allowed_chats.len())
            .field("db_path", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_token_returns_missing_token_error() {
        assert_eq!(Config::from_token(None), Err(ConfigError::MissingToken));
    }

    #[test]
    fn empty_token_returns_empty_token_error() {
        let res = Config::from_token(Some(String::new()));
        assert_eq!(res, Err(ConfigError::EmptyToken));
    }

    #[test]
    fn whitespace_token_returns_empty_token_error() {
        let res = Config::from_token(Some(" \t\n ".into()));
        assert_eq!(res, Err(ConfigError::EmptyToken));
    }

    #[test]
    fn valid_token_returns_config() -> Result<(), ConfigError> {
        let token = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11";
        let cfg = Config::from_token(Some(token.into()))?;
        assert_eq!(cfg.telegram_token(), token);
        assert_eq!(cfg.challenge_ttl().as_secs(), DEFAULT_CHALLENGE_TTL_SECS);
        assert_eq!(cfg.max_attempts(), DEFAULT_MAX_ATTEMPTS);
        assert!(cfg.is_chat_allowed(ChatId(123)));
        assert_eq!(cfg.db_path(), std::path::Path::new(DEFAULT_DB_PATH));
        Ok(())
    }

    #[test]
    fn custom_valid_policy_and_is_chat_allowed() -> Result<(), ConfigError> {
        let (c, db) = (Some("-1001234567890, -1009876543210,"), "/tmp/s.redb");
        let cfg = Config::from_env_vars(Some("tok".into()), Some("60"), Some("3"), c, Some(db))?;
        assert_eq!(cfg.challenge_ttl().as_secs(), 60);
        assert_eq!(cfg.max_attempts(), 3);
        assert!(cfg.is_chat_allowed(ChatId(-1_001_234_567_890)));
        assert!(cfg.is_chat_allowed(ChatId(-1_009_876_543_210)));
        assert!(!cfg.is_chat_allowed(ChatId(999)));
        assert_eq!(cfg.db_path(), Path::new(db));
        Ok(())
    }

    #[test]
    fn invalid_ttl_returns_error() {
        for v in ["0", "301", "abc"] {
            let res = Config::from_env_vars(Some("t".into()), Some(v), None, None, None);
            assert_eq!(res, Err(ConfigError::InvalidChallengeTtl));
        }
    }

    #[test]
    fn invalid_max_attempts_returns_error() {
        for v in ["0", "6", "abc"] {
            let res = Config::from_env_vars(Some("t".into()), None, Some(v), None, None);
            assert_eq!(res, Err(ConfigError::InvalidMaxAttempts));
        }
    }

    #[test]
    fn invalid_allowed_chats_returns_error() {
        for v in ["abc", ","] {
            let res = Config::from_env_vars(Some("t".into()), None, None, Some(v), None);
            assert_eq!(res, Err(ConfigError::InvalidAllowedChats));
        }
    }

    #[test]
    fn empty_or_whitespace_db_path_returns_error() {
        for v in ["", " \t\n "] {
            let res = Config::from_env_vars(Some("t".into()), None, None, None, Some(v));
            assert_eq!(res, Err(ConfigError::InvalidDbPath));
        }
    }

    #[test]
    fn debug_format_redacts_token_and_db_path() -> Result<(), ConfigError> {
        let (t, db) = ("secret_token_xyz", "/tmp/secret/db.redb");
        let cfg =
            Config::from_env_vars(Some(t.into()), Some("120"), Some("4"), Some("-1"), Some(db))?;
        let dbg = format!("{cfg:?}");
        assert!(dbg.contains("[REDACTED]") && !dbg.contains(t) && !dbg.contains("-1"));
        assert!(!dbg.contains(db) && dbg.contains("db_path: \"[REDACTED]\""));
        assert!(dbg.contains("allowed_chats_count: 1") && dbg.contains("challenge_ttl_secs: 120"));
        assert!(dbg.contains("max_attempts: 4"));
        Ok(())
    }
}
