use crate::monitor::Monitor;

use super::BotState;

impl BotState {
    pub async fn install_monitor(
        &self,
        state: &String,
        access_token: String,
        refresh_token: String,
    ) -> Result<u64, String> {
        let (guild_id, character_id) = {
            let this = self.0.read().await;
            let pending = this.pending.get(state).unwrap();
            (
                pending.interaction.guild_id.unwrap().get(),
                pending.interaction.user.id.get(),
            )
        };

        Monitor::install(
            self.clone(),
            access_token,
            refresh_token,
            guild_id,
            character_id,
        )
        .await
    }

    pub async fn has_eve_character(&self, eve_character_id: u64) -> bool {
        let this = self.0.read().await;
        let monitor = this.monitor.as_ref().unwrap();
        monitor.has_eve_character(eve_character_id).await
    }

    pub async fn refresh_eve_character(
        &self,
        eve_character_id: u64,
        refresh_token: &String,
    ) -> Result<u64, String> {
        let this = self.0.read().await;
        let monitor = this.monitor.as_ref().unwrap();
        monitor.refresh_eve_character(eve_character_id, refresh_token).await
    }

    pub async fn create_eve_character(
        &self,
        refresh_token: String,
        eve_character_id: u64,
        eve_character_name: String,
        discord_character_id: u64,
        discord_guild_id: u64,
        discord_channel_id: u64,
        discord_activity_thread_id: u64,
    ) {
        let this = self.0.read().await;
        let monitor = this.monitor.as_ref().unwrap();
        monitor
            .create_eve_character(
                refresh_token,
                eve_character_id,
                eve_character_name,
                discord_character_id,
                discord_guild_id,
                discord_channel_id,
                discord_activity_thread_id,
            )
            .await;
    }
}
