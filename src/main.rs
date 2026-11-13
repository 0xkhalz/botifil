mod config;

use teloxide::adaptors::throttle::Limits;
use teloxide::prelude::*;
use teloxide::requests::Requester;

/// Verifies the bot token by calling `getMe` and returns the bot's username.
///
/// # Errors
///
/// Returns [`teloxide::RequestError`] if the Telegram Bot API call fails.
pub async fn check_bot<B>(bot: &B) -> Result<String, teloxide::RequestError>
where
    B: Requester<Err = teloxide::RequestError>,
{
    let me = bot.get_me().await?;
    let username = me.username().to_string();
    tracing::info!(username = %username, "Telegram bot authorized");
    Ok(username)
}

async fn wait_for_shutdown() -> std::io::Result<()> {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        res = tokio::signal::ctrl_c() => {
            res?;
            tracing::info!("Received SIGINT, initiating graceful shutdown");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM, initiating graceful shutdown");
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let config = config::Config::from_env()?;
    let bot = Bot::new(config.telegram_token());
    let throttled = bot.throttle(Limits::default());

    check_bot(&throttled).await?;

    wait_for_shutdown().await?;
    tracing::info!("Shutdown complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn get_me_success_returns_bot_username() -> Result<(), Box<dyn std::error::Error>> {
        let server = MockServer::start().await;
        Mock::given(path("/bottest_token/GetMe"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(
                        r#"{"ok":true,"result":{"id":123456789,"is_bot":true,"first_name":"Botifil Test Bot","username":"botifil_test_bot","can_join_groups":true,"can_read_all_group_messages":false,"supports_inline_queries":false,"has_main_web_app":false}}"#,
                    ),
            )
            .mount(&server)
            .await;

        let url = Url::parse(&server.uri())?;
        let bot = Bot::new("test_token").set_api_url(url);
        let throttled = bot.throttle(Limits::default());
        let username = check_bot(&throttled).await?;
        assert_eq!(username, "botifil_test_bot");
        Ok(())
    }

    #[tokio::test]
    async fn get_me_unauthorized_returns_error() -> Result<(), Box<dyn std::error::Error>> {
        let server = MockServer::start().await;
        Mock::given(path("/bottest_token/GetMe"))
            .respond_with(
                ResponseTemplate::new(401)
                    .insert_header("content-type", "application/json")
                    .set_body_string(
                        r#"{"ok":false,"error_code":401,"description":"Unauthorized"}"#,
                    ),
            )
            .mount(&server)
            .await;

        let url = Url::parse(&server.uri())?;
        let bot = Bot::new("test_token").set_api_url(url);
        let throttled = bot.throttle(Limits::default());
        let res = check_bot(&throttled).await;
        assert!(res.is_err());
        Ok(())
    }
}
