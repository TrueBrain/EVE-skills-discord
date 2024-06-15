use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

use crate::state::BotState;

#[derive(Debug, Deserialize)]
struct AuthRequest {
    code: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct AuthLogin {
    state: String,
}

async fn url_login(
    Query(query): Query<AuthLogin>,
    State(bot): State<BotState>,
) -> impl IntoResponse {
    if !bot.pending_exists(&query.state).await {
        return (
            StatusCode::BAD_REQUEST,
            "Your token expired. Use /monitor on Discord to try again.",
        )
            .into_response();
    }

    let auth_url = bot.get_authorization_url(query.state).await;

    Redirect::to(auth_url.as_ref()).into_response()
}

async fn url_oauth_callback(
    Query(query): Query<AuthRequest>,
    State(bot): State<BotState>,
) -> impl IntoResponse {
    if !bot.pending_exists(&query.state).await {
        return (
            StatusCode::BAD_REQUEST,
            "Your token expired. Use /monitor on Discord to try again.",
        )
            .into_response();
    }

    let tokens = bot.exchange_code(query.code).await;

    let reply = if let Ok((access_token, refresh_token)) = tokens {
        tokio::spawn(async move {
            bot.pending_edit_response(
                &query.state,
                &format!("Authenticated successful. Creating channel ..."),
            )
            .await;

            let discord_channel_id = bot
                .install_monitor(&query.state, access_token, refresh_token)
                .await;

            match discord_channel_id {
                Ok(channel_id) => {
                    bot.pending_edit_response(
                        &query.state,
                        &format!("Your character is now monitored in <#{}>", channel_id),
                    )
                    .await;
                }
                Err(error) => {
                    bot.pending_edit_response(
                        &query.state,
                        &format!("Failed to create channel: {}", error),
                    )
                    .await;
                }
            }

            bot.pending_remove(&query.state).await;
        });

        (StatusCode::OK, "You are now authenticated. Check Discord for next steps. You can now safely close this tab.")
    } else {
        tokio::spawn(async move {
            bot.pending_edit_response(
                &query.state,
                &format!("Authentication failed. Use /monitor to try again."),
            )
            .await;

            bot.pending_remove(&query.state).await;
        });

        (
            StatusCode::OK,
            "Authentication failed. Use /monitor on Discord to try again. You can now safely close this tab.",
        )
    };

    reply.into_response()
}

pub fn create_app(bot: BotState) -> Router {
    Router::new()
        .route("/callback", get(url_oauth_callback))
        .route("/login", get(url_login))
        .with_state(bot)
}
