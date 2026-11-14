use std::{sync::Arc, time::Duration};

use botifil::{config, domain::SessionOutcome, handlers, store::SessionStore};
use teloxide::{
    adaptors::{Throttle, throttle::Limits},
    dispatching::Dispatcher,
    dptree,
    error_handlers::LoggingErrorHandler,
    prelude::*,
    requests::Requester,
    types::AllowedUpdate,
    update_listeners::Polling,
};
use tokio::sync::watch;

/// Verifies the bot token by calling `getMe` and returns the bot's username.
///
/// # Errors
/// Returns [`teloxide::RequestError`] if the Telegram Bot API call fails.
pub async fn check_bot<B: Requester<Err = teloxide::RequestError>>(
    bot: &B,
) -> Result<String, teloxide::RequestError> {
    let username = bot.get_me().await?.username().to_string();
    tracing::info!(username = %username, "Telegram bot authorized");
    Ok(username)
}

#[must_use]
pub fn allowed_update_types() -> Vec<AllowedUpdate> {
    use AllowedUpdate as A;
    vec![A::ChatJoinRequest, A::CallbackQuery, A::ChatMember]
}

pub async fn run_expiry_reaper(store: SessionStore, mut shutdown: watch::Receiver<bool>) {
    let mut ticker = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let exp = store.reap_expired(tokio::time::Instant::now()).await;
                if !exp.is_empty() {
                    tracing::info!(
                        count = exp.len(),
                        outcome = ?SessionOutcome::LeftPendingExpired,
                        "Evicted expired sessions; leaving join requests pending"
                    );
                }
            }
            res = shutdown.changed() => if res.is_err() || *shutdown.borrow() { break; },
        }
    }
}

async fn wait_for_shutdown() -> std::io::Result<()> {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        res = tokio::signal::ctrl_c() => {
            res?;
            tracing::info!("Received SIGINT, initiating graceful shutdown");
        }
        _ = sigterm.recv() => tracing::info!("Received SIGTERM, initiating graceful shutdown"),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let cfg = Arc::new(config::Config::from_env()?);
    let store = SessionStore::default();
    let throttled = Bot::new(cfg.telegram_token()).throttle(Limits::default());
    check_bot(&throttled).await?;

    let mut dp = Dispatcher::builder(throttled.clone(), handlers::schema::<Throttle<Bot>>())
        .dependencies(dptree::deps![cfg, store.clone()])
        .build();

    let (tok, (tx, rx)) = (dp.shutdown_token(), watch::channel(false));
    let reaper = tokio::spawn(run_expiry_reaper(store, rx));
    tokio::spawn(async move {
        let _ = wait_for_shutdown().await;
        let _ = tx.send(true);
        if let Ok(f) = tok.shutdown() {
            f.await;
        }
    });

    let poller = Polling::builder(throttled);
    let listener = poller.allowed_updates(allowed_update_types()).build();
    let err_handler = LoggingErrorHandler::new();
    dp.try_dispatch_with_listener(listener, err_handler).await?;
    let _ = reaper.await;
    tracing::info!("Shutdown complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    type TestRes<T> = Result<T, Box<dyn std::error::Error>>;
    const BOT_JSON: &str = r#"{"ok":true,"result":{"id":1,"is_bot":true,"first_name":"B","username":"botifil_test_bot","can_join_groups":true,"can_read_all_group_messages":false,"supports_inline_queries":false,"has_main_web_app":false}}"#;
    const ERR_JSON: &str = r#"{"ok":false,"error_code":401,"description":"Unauthorized"}"#;

    #[test]
    fn allowed_updates_includes_join_callback_and_member() {
        use AllowedUpdate::{CallbackQuery as C, ChatJoinRequest as J, ChatMember as M};
        let u = allowed_update_types();
        assert!(u.contains(&J) && u.contains(&C) && u.contains(&M));
    }

    async fn mock(code: u16, body: &str) -> TestRes<(MockServer, Throttle<Bot>)> {
        let s = MockServer::start().await;
        let resp = ResponseTemplate::new(code).set_body_string(body);
        let mock = Mock::given(path("/bottest_token/GetMe")).respond_with(resp);
        mock.mount(&s).await;
        let bot = Bot::new("test_token").set_api_url(Url::parse(&s.uri())?);
        Ok((s, bot.throttle(Limits::default())))
    }

    #[tokio::test]
    async fn get_me_success_returns_bot_username() -> TestRes<()> {
        let (_s, bot) = mock(200, BOT_JSON).await?;
        assert_eq!(check_bot(&bot).await?, "botifil_test_bot");
        Ok(())
    }

    #[tokio::test]
    async fn get_me_unauthorized_returns_error() -> TestRes<()> {
        let (_s, bot) = mock(401, ERR_JSON).await?;
        assert!(check_bot(&bot).await.is_err());
        Ok(())
    }
}
