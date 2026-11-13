use std::fmt;

/// Configuration errors for bot initialization.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("TELOXIDE_TOKEN environment variable is not set")]
    MissingToken,
    #[error("TELOXIDE_TOKEN environment variable cannot be empty or whitespace")]
    EmptyToken,
}

/// Runtime bot configuration holding the Telegram bot token.
#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    telegram_token: String,
}

impl Config {
    /// Loads configuration from environment variables and `.env` file.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::MissingToken`] if `TELOXIDE_TOKEN` is not set,
    /// or [`ConfigError::EmptyToken`] if it is empty or whitespace.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();
        Self::from_token(std::env::var("TELOXIDE_TOKEN").ok())
    }

    /// Creates a configuration from an optional token string.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::MissingToken`] if the token is `None`,
    /// or [`ConfigError::EmptyToken`] if it is empty or whitespace.
    pub fn from_token(token: Option<String>) -> Result<Self, ConfigError> {
        match token {
            None => Err(ConfigError::MissingToken),
            Some(token) if token.trim().is_empty() => Err(ConfigError::EmptyToken),
            Some(token) => Ok(Self {
                telegram_token: token,
            }),
        }
    }

    /// Returns the Telegram bot token.
    #[must_use]
    pub fn telegram_token(&self) -> &str {
        &self.telegram_token
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("telegram_token", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_token_returns_missing_token_error() {
        let res = Config::from_token(None);
        assert_eq!(res, Err(ConfigError::MissingToken));
    }

    #[test]
    fn empty_token_returns_empty_token_error() {
        let res = Config::from_token(Some(String::new()));
        assert_eq!(res, Err(ConfigError::EmptyToken));
    }

    #[test]
    fn whitespace_token_returns_empty_token_error() {
        let res = Config::from_token(Some("   \t\n  ".to_string()));
        assert_eq!(res, Err(ConfigError::EmptyToken));
    }

    #[test]
    fn valid_token_returns_config() -> Result<(), ConfigError> {
        let token = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11".to_string();
        let config = Config::from_token(Some(token.clone()))?;
        assert_eq!(config.telegram_token(), &token);
        Ok(())
    }

    #[test]
    fn debug_format_redacts_token() -> Result<(), ConfigError> {
        let token = "secret_telegram_token_xyz".to_string();
        let config = Config::from_token(Some(token.clone()))?;
        let debug_output = format!("{config:?}");
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains(&token));
        Ok(())
    }
}
