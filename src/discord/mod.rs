use serenity::all::{ActivityData, Command, GuildId, Interaction, Ready};
use serenity::{async_trait, prelude::*};
use tracing::{error, info};

use crate::monitor::Monitor;
use crate::state::BotState;

mod monitor;

#[async_trait]
impl EventHandler for BotState {
    async fn ready(&self, _: Context, ready: Ready) {
        info!("Connected to Discord as {}", ready.user.name);
    }

    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        if !self.set_if_startup().await {
            return;
        }
        self.set_discord(ctx.clone()).await;

        ctx.set_activity(Some(ActivityData::custom("Monitoring your skills")));

        /* Register our global commands; we only use ephemeral responses, so we don't care what channel it is done in. */
        if let Err(why) = Command::create_global_command(&ctx.http, monitor::register()).await {
            error!("Error creating global command: {why:?}");
        }

        Monitor::start(self.clone()).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            match command.data.name.as_str() {
                "monitor" => monitor::run(&self, &ctx, &command).await,
                _ => {}
            };
        }
    }
}
