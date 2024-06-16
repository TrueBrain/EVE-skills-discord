use serenity::all::{CommandInteraction, EditInteractionResponse};

use super::{BotState, PendingState};

impl BotState {
    pub async fn pending_create(&self, key: String, value: &CommandInteraction) {
        self.0.write().await.pending.insert(
            key,
            PendingState {
                expire_time: std::time::Instant::now() + std::time::Duration::from_secs(300),
                interaction: value.clone(),
            },
        );
    }

    pub async fn pending_exists(&self, key: &str) -> bool {
        self.0.read().await.pending.contains_key(key)
    }

    pub async fn pending_edit_response(&self, key: &str, message: &String) {
        let this = self.0.read().await;
        let value = this.pending.get(key).unwrap();

        let _ = value
            .interaction
            .edit_response(
                &this.discord.as_ref().unwrap().http,
                EditInteractionResponse::new().content(message),
            )
            .await;
    }

    pub async fn pending_remove(&self, key: &str) {
        self.0.write().await.pending.remove(key);
    }
}
