use std::env;

use oauth2::CsrfToken;
use serenity::all::{
    CommandInteraction, Context, CreateCommand, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};

use crate::state::BotState;

pub async fn run(bot: &BotState, ctx: &Context, command: &CommandInteraction) {
    let state = CsrfToken::new_random();
    bot.pending_create(state.secret().clone(), command).await;

    let webserver_url =
        env::var("WEBSERVER_URL").expect("Expected WEBSERVER_URL in the environment");

    let message = format!(
        "Visit {}/login?state={} to authenticate an EVE Online character to monitor.",
        webserver_url,
        state.secret()
    );

    let _ = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(message)
                    .ephemeral(true),
            ),
        )
        .await;
}

pub fn register() -> CreateCommand {
    CreateCommand::new("monitor").description("Monitor skills for an EVE character.")
}
