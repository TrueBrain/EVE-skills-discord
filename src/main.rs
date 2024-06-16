use std::env;

use serenity::all::GatewayIntents;
use serenity::Client;
use tracing::error;

mod discord;
mod esi;
mod monitor;
mod state;
mod webserver;

#[tokio::main]
async fn main() {
    /* Load, if it exists, from the .env file. This mostly makes development easier. */
    let _ = dotenv::dotenv();
    let discord_token =
        env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in the environment");

    tracing_subscriber::fmt::init();

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILDS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let bot =
        state::BotState::new("Authentication timed out. Use /monitor to try again.".to_string());

    let web_app = webserver::create_app(bot.clone());
    let web_listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tokio::spawn(async move {
        axum::serve(web_listener, web_app).await.unwrap();
    });

    let mut discord_client = Client::builder(&discord_token, intents)
        .event_handler(bot)
        .await
        .expect("Error creating client");

    if let Err(why) = discord_client.start().await {
        error!("Client error: {why:?}");
    }
}
