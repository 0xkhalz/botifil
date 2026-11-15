use super::*;
use ConfigError::{
    EmptyToken, InvalidAllowedChats, InvalidChallengeTtl, InvalidDbPath, InvalidMaxAttempts,
    InvalidWebappBind, InvalidWebappUrl, MissingToken,
};

fn wa(url: Option<&str>, bind: Option<&str>) -> Result<Config, ConfigError> {
    Config::from_all_env_vars(Some("tok".into()), None, None, None, None, url, bind)
}

#[test]
fn valid_token_and_defaults() -> Result<(), ConfigError> {
    let token = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11";
    let cfg = Config::from_token(Some(token.into()))?;
    assert_eq!(cfg.telegram_token(), token);
    assert_eq!(cfg.challenge_ttl().as_secs(), DEFAULT_CHALLENGE_TTL_SECS);
    assert_eq!(cfg.max_attempts(), DEFAULT_MAX_ATTEMPTS);
    assert!(cfg.is_chat_allowed(ChatId(123)));
    assert_eq!(cfg.db_path(), Path::new(DEFAULT_DB_PATH));
    assert!(cfg.webapp_url().is_none());
    assert_eq!(cfg.webapp_bind(), DEFAULT_WEBAPP_BIND);
    Ok(())
}

#[test]
fn custom_valid_policy_and_is_chat_allowed() -> Result<(), ConfigError> {
    let (c, db) = (Some("-1001234567890, -1009876543210,"), "/tmp/s.redb");
    let cfg = Config::from_env_vars(Some("tok".into()), Some("60"), Some("3"), c, Some(db))?;
    assert_eq!((cfg.challenge_ttl().as_secs(), cfg.max_attempts()), (60, 3));
    assert!(cfg.is_chat_allowed(ChatId(-1_001_234_567_890)));
    assert!(cfg.is_chat_allowed(ChatId(-1_009_876_543_210)));
    assert!(!cfg.is_chat_allowed(ChatId(999)));
    assert_eq!(cfg.db_path(), Path::new(db));
    Ok(())
}

#[test]
fn token_and_policy_validation_errors() {
    let env = |t, a, c, db| Config::from_env_vars(Some("t".into()), t, a, c, db);
    assert_eq!(Config::from_token(None), Err(MissingToken));
    assert_eq!(Config::from_token(Some(String::new())), Err(EmptyToken));
    assert_eq!(Config::from_token(Some(" \t\n ".into())), Err(EmptyToken));
    for v in ["0", "301", "abc"] {
        assert_eq!(env(Some(v), None, None, None), Err(InvalidChallengeTtl));
        assert_eq!(env(None, Some(v), None, None), Err(InvalidMaxAttempts));
    }
    assert_eq!(env(None, Some("6"), None, None), Err(InvalidMaxAttempts));
    for v in ["abc", ","] {
        assert_eq!(env(None, None, Some(v), None), Err(InvalidAllowedChats));
    }
    for v in ["", " \t\n "] {
        assert_eq!(env(None, None, None, Some(v)), Err(InvalidDbPath));
    }
}

#[test]
fn webapp_url_and_bind_validation() -> Result<(), ConfigError> {
    let url = "https://bot.example.com/verify";
    let https = wa(Some(url), None)?;
    assert_eq!(https.webapp_url().map(Url::as_str), Some(url));
    assert_eq!(https.webapp_bind(), DEFAULT_WEBAPP_BIND);

    for u in [
        "http://localhost:8080",
        "http://127.0.0.1:3000",
        "http://[::1]:8080",
    ] {
        assert!(wa(Some(u), None)?.webapp_url().is_some());
    }
    assert_eq!(wa(None, Some(""))?.webapp_bind(), DEFAULT_WEBAPP_BIND);
    assert_eq!(wa(None, Some("0.0.0.0:9090"))?.webapp_bind().port(), 9090);

    let bad_urls = [
        "",
        "  ",
        "http://example.com",
        "http://127.0.0.2:8080",
        "ftp://localhost",
        "mailto:a@b",
        "https://user:pass@example.com",
        "bad",
    ];
    for u in bad_urls {
        assert_eq!(wa(Some(u), None), Err(InvalidWebappUrl));
    }
    for b in ["localhost:8080", "127.0.0.1", "bad", "127.0.0.1:99999"] {
        assert_eq!(wa(None, Some(b)), Err(InvalidWebappBind));
    }
    Ok(())
}

#[test]
fn debug_format_redacts_token_chats_and_db_path() -> Result<(), ConfigError> {
    let (t, db) = ("secret_token_xyz", "/tmp/secret/db.redb");
    let (u, b) = (Some("https://bot.example.com"), Some("127.0.0.1:8081"));
    let cfg = Config::from_all_env_vars(
        Some(t.into()),
        Some("120"),
        Some("4"),
        Some("-1001"),
        Some(db),
        u,
        b,
    )?;
    let dbg = format!("{cfg:?}");
    assert!(dbg.contains("[REDACTED]") && !dbg.contains(t) && !dbg.contains("-1001"));
    assert!(!dbg.contains(db) && dbg.contains("db_path: \"[REDACTED]\""));
    assert!(dbg.contains("allowed_chats_count: 1") && dbg.contains("challenge_ttl_secs: 120"));
    assert!(dbg.contains("max_attempts: 4") && dbg.contains("127.0.0.1:8081"));
    Ok(())
}
