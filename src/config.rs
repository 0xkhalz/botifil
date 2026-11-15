use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4};
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

fn parse_webapp_url(raw: Option<&str>) -> Result<Option<Url>, ConfigError> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let u = Url::parse(s).map_err(|_| ConfigError::InvalidWebappUrl)?;
    let is_local = matches!(
        u.host(),
        Some(
            Host::Domain("localhost")
                | Host::Ipv4(Ipv4Addr::LOCALHOST)
                | Host::Ipv6(Ipv6Addr::LOCALHOST)
        )
    );
    let ok = u.has_host() && (u.scheme() == "https" || (u.scheme() == "http" && is_local));
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
    /// # Errors
    /// Returns [`ConfigError`] if any environment variable has an invalid value.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();
        let v = |k| std::env::var(k).ok();
        Self::from_all_env_vars(
            v("TELOXIDE_TOKEN"),
            v("BOTIFIL_CHALLENGE_TTL_SECS").as_deref(),
            v("BOTIFIL_MAX_ATTEMPTS").as_deref(),
            v("BOTIFIL_ALLOWED_CHATS").as_deref(),
            v("BOTIFIL_WEBAPP_URL").as_deref(),
            v("BOTIFIL_WEBAPP_BIND").as_deref(),
        )
    }

    /// Creates a configuration from an optional token string with default policy.
    /// # Errors
    /// Returns [`ConfigError::MissingToken`] or [`ConfigError::EmptyToken`] on invalid token.
    pub fn from_token(token: Option<String>) -> Result<Self, ConfigError> {
        Self::from_env_vars(token, None, None, None)
    }

    /// Creates a configuration from raw environment variable values using default `WebApp` settings.
    /// # Errors
    /// Returns [`ConfigError`] if any setting fails validation or parsing.
    pub fn from_env_vars(
        token: Option<String>,
        ttl: Option<&str>,
        max_attempts: Option<&str>,
        allowed_chats: Option<&str>,
    ) -> Result<Self, ConfigError> {
        Self::from_all_env_vars(token, ttl, max_attempts, allowed_chats, None, None)
    }

    /// Creates a configuration from all raw environment variable values.
    /// # Errors
    /// Returns [`ConfigError`] if any setting fails validation or parsing.
    pub fn from_all_env_vars(
        token: Option<String>,
        ttl: Option<&str>,
        max_attempts: Option<&str>,
        allowed_chats: Option<&str>,
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
            webapp_url: parse_webapp_url(webapp_url)?,
            webapp_bind: parse_webapp_bind(webapp_bind)?,
        })
    }

    #[must_use]
    pub fn telegram_token(&self) -> &str {
        &self.telegram_token
    }
    #[must_use]
    pub fn challenge_ttl(&self) -> Duration {
        Duration::from_secs(self.challenge_ttl_secs)
    }
    #[must_use]
    pub fn max_attempts(&self) -> u8 {
        self.max_attempts
    }
    #[must_use]
    pub fn is_chat_allowed(&self, chat_id: ChatId) -> bool {
        self.allowed_chats.is_empty() || self.allowed_chats.contains(&chat_id)
    }
    #[must_use]
    pub fn webapp_url(&self) -> Option<&Url> {
        self.webapp_url.as_ref()
    }
    #[must_use]
    pub const fn webapp_bind(&self) -> SocketAddr {
        self.webapp_bind
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("telegram_token", &"[REDACTED]")
            .field("challenge_ttl_secs", &self.challenge_ttl_secs)
            .field("max_attempts", &self.max_attempts)
            .field("allowed_chats_count", &self.allowed_chats.len())
            .field("webapp_url", &self.webapp_url.as_ref().map(Url::as_str))
            .field("webapp_bind", &self.webapp_bind)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ConfigError::{
        EmptyToken, InvalidAllowedChats, InvalidChallengeTtl, InvalidMaxAttempts,
        InvalidWebappBind, InvalidWebappUrl, MissingToken,
    };

    fn wa(url: Option<&str>, bind: Option<&str>) -> Result<Config, ConfigError> {
        Config::from_all_env_vars(Some("tok".into()), None, None, None, url, bind)
    }

    #[test]
    fn valid_defaults_and_custom_policy() -> Result<(), ConfigError> {
        let tok = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11";
        let def = Config::from_token(Some(tok.into()))?;
        assert_eq!(def.telegram_token(), tok);
        assert_eq!(def.challenge_ttl().as_secs(), DEFAULT_CHALLENGE_TTL_SECS);
        assert_eq!(def.max_attempts(), DEFAULT_MAX_ATTEMPTS);
        assert!(def.is_chat_allowed(ChatId(123)));
        assert!(def.webapp_url().is_none() && def.webapp_bind() == DEFAULT_WEBAPP_BIND);

        let chats = Some("-1001234567890, -1009876543210,");
        let cfg = Config::from_env_vars(Some("tok".into()), Some("60"), Some("3"), chats)?;
        assert_eq!((cfg.challenge_ttl().as_secs(), cfg.max_attempts()), (60, 3));
        assert!(cfg.is_chat_allowed(ChatId(-1_001_234_567_890)));
        assert!(cfg.is_chat_allowed(ChatId(-1_009_876_543_210)));
        assert!(!cfg.is_chat_allowed(ChatId(999)));
        Ok(())
    }

    #[test]
    fn validation_errors_for_token_and_policy() {
        let env = |t, a, c| Config::from_env_vars(Some("t".into()), t, a, c);
        assert_eq!(Config::from_token(None), Err(MissingToken));
        assert_eq!(Config::from_token(Some(String::new())), Err(EmptyToken));
        assert_eq!(Config::from_token(Some(" \t\n ".into())), Err(EmptyToken));
        for v in ["0", "301", "abc"] {
            assert_eq!(env(Some(v), None, None), Err(InvalidChallengeTtl));
            assert_eq!(env(None, Some(v), None), Err(InvalidMaxAttempts));
        }
        assert_eq!(env(None, Some("6"), None), Err(InvalidMaxAttempts));
        for v in ["abc", ","] {
            assert_eq!(env(None, None, Some(v)), Err(InvalidAllowedChats));
        }
    }

    #[test]
    fn webapp_url_and_bind_validation() -> Result<(), ConfigError> {
        let url = "https://bot.example.com/verify";
        let https = wa(Some(url), None)?;
        assert_eq!(https.webapp_url().map(Url::as_str), Some(url));
        assert_eq!(https.webapp_bind(), DEFAULT_WEBAPP_BIND);
        let ok = "http://localhost:8080 http://127.0.0.1:3000 http://[::1]:8080";
        for u in ok.split(' ') {
            assert!(wa(Some(u), None)?.webapp_url().is_some());
        }
        assert_eq!(wa(Some("  "), Some(""))?.webapp_url(), None);
        assert_eq!(wa(None, Some("0.0.0.0:9090"))?.webapp_bind().port(), 9090);
        let bad = "http://example.com http://127.0.0.2:8080 ftp://localhost mailto:a@b bad";
        for u in bad.split(' ') {
            assert_eq!(wa(Some(u), None), Err(InvalidWebappUrl));
        }
        for b in ["localhost:8080", "127.0.0.1", "bad", "127.0.0.1:99999"] {
            assert_eq!(wa(None, Some(b)), Err(InvalidWebappBind));
        }
        Ok(())
    }

    #[test]
    fn debug_format_redacts_token() -> Result<(), ConfigError> {
        let tok = "secret_telegram_token_xyz".to_string();
        let (u, b) = (Some("https://bot.example.com"), Some("127.0.0.1:8081"));
        let all = Config::from_all_env_vars;
        let cfg = all(
            Some(tok.clone()),
            Some("120"),
            Some("4"),
            Some("-1001"),
            u,
            b,
        )?;
        let dbg = format!("{cfg:?}");
        assert!(dbg.contains("[REDACTED]") && !dbg.contains(&tok) && !dbg.contains("-1001"));
        assert!(dbg.contains("allowed_chats_count: 1") && dbg.contains("challenge_ttl_secs: 120"));
        assert!(dbg.contains("max_attempts: 4") && dbg.contains("127.0.0.1:8081"));
        Ok(())
    }
}
