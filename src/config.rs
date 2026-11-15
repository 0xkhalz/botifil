use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::time::Duration;
use teloxide::types::ChatId;
use url::{Host, Url};

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
/// Default bind address for the embedded `WebApp` HTTP server.
pub const DEFAULT_WEBAPP_BIND: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080));

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
    #[error("BOTIFIL_WEBAPP_URL must use https:// (or http:// on localhost/127.0.0.1/::1)")]
    InvalidWebappUrl,
    #[error("BOTIFIL_WEBAPP_BIND must be a valid socket address (e.g. 127.0.0.1:8080)")]
    InvalidWebappBind,
}

/// Runtime bot configuration holding the Telegram bot token and policy settings.
#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    telegram_token: String,
    challenge_ttl_secs: u64,
    max_attempts: u8,
    allowed_chats: Vec<ChatId>,
    db_path: PathBuf,
    webapp_url: Option<Url>,
    webapp_bind: SocketAddr,
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

fn parse_webapp_url(raw: Option<&str>) -> Result<Option<Url>, ConfigError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let s = raw.trim();
    if s.is_empty() {
        return Err(ConfigError::InvalidWebappUrl);
    }
    let u = Url::parse(s).map_err(|_| ConfigError::InvalidWebappUrl)?;
    let is_local = matches!(
        u.host(),
        Some(
            Host::Domain("localhost")
                | Host::Ipv4(Ipv4Addr::LOCALHOST)
                | Host::Ipv6(Ipv6Addr::LOCALHOST)
        )
    );
    let ok = u.has_host()
        && u.username().is_empty()
        && u.password().is_none()
        && (u.scheme() == "https" || (u.scheme() == "http" && is_local));
    ok.then_some(Some(u)).ok_or(ConfigError::InvalidWebappUrl)
}

fn parse_webapp_bind(raw: Option<&str>) -> Result<SocketAddr, ConfigError> {
    match raw.map(str::trim) {
        None | Some("") => Ok(DEFAULT_WEBAPP_BIND),
        Some(s) => s.parse().map_err(|_| ConfigError::InvalidWebappBind),
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
        Self::from_all_env_vars(
            v("TELOXIDE_TOKEN"),
            v("BOTIFIL_CHALLENGE_TTL_SECS").as_deref(),
            v("BOTIFIL_MAX_ATTEMPTS").as_deref(),
            v("BOTIFIL_ALLOWED_CHATS").as_deref(),
            v("BOTIFIL_DB_PATH").as_deref(),
            v("BOTIFIL_WEBAPP_URL").as_deref(),
            v("BOTIFIL_WEBAPP_BIND").as_deref(),
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

    /// Creates a configuration from raw environment variable values using default `WebApp` settings.
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
        Self::from_all_env_vars(token, ttl, max_attempts, allowed_chats, db_path, None, None)
    }

    /// Creates a configuration from all raw environment variable values.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if any setting fails validation or parsing.
    pub fn from_all_env_vars(
        token: Option<String>,
        ttl: Option<&str>,
        max_attempts: Option<&str>,
        allowed_chats: Option<&str>,
        db_path: Option<&str>,
        webapp_url: Option<&str>,
        webapp_bind: Option<&str>,
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
            webapp_url: parse_webapp_url(webapp_url)?,
            webapp_bind: parse_webapp_bind(webapp_bind)?,
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

    /// Returns the validated optional `WebApp` base URL.
    #[must_use]
    pub fn webapp_url(&self) -> Option<&Url> {
        self.webapp_url.as_ref()
    }

    /// Returns the listen address for the embedded `WebApp` HTTP server.
    #[must_use]
    pub const fn webapp_bind(&self) -> SocketAddr {
        self.webapp_bind
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
            .field("webapp_url", &self.webapp_url.as_ref().map(Url::as_str))
            .field("webapp_bind", &self.webapp_bind)
            .finish()
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
