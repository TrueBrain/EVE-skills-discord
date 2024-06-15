use std::env;

use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{monitor::Monitor, state::BotState};

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    sub: String,
    name: String,
}

fn decode_jwt(token: &str) -> Result<Claims, ()> {
    let mut validation = Validation::default();
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.validate_aud = false;

    let claims = decode::<Claims>(&token, &DecodingKey::from_secret(&[]), &validation);

    match claims {
        Ok(claims) => Ok(claims.claims),
        Err(_) => Err(()),
    }
}

impl Monitor {
    pub async fn install(
        bot: BotState,
        access_token: String,
        refresh_token: String,
        discord_guild_id: u64,
        discord_character_id: u64,
    ) -> Result<u64, String> {
        let claims = decode_jwt(&access_token);

        if let Ok(claims) = claims {
            /* The ID is prefixed with "CHARACTER:EVE:" */
            let eve_character_id: u64 = claims.sub.split(':').nth(2).unwrap().parse().unwrap();

            /* Check if we already know this character-id. */
            if bot.has_eve_character(eve_character_id).await {
                return Err("This character is already actively monitored.".to_string());
            }
            /* Check if this was an expired entry. */
            if let Ok(discord_channel_id) = bot.refresh_eve_character(eve_character_id, &refresh_token).await {
                return Ok(discord_channel_id);
            }

            let discord_category_id: u64 = env::var("DISCORD_CATEGORY_ID")
                .expect("Expected DISCORD_CATEGORY_ID in the environment")
                .parse()
                .unwrap();
            let (discord_channel_id, discord_activity_thread_id) = bot
                .discord_create_private_channel(
                    discord_guild_id,
                    discord_category_id,
                    discord_character_id,
                    &claims.name,
                )
                .await?;

            bot.discord_send_message(discord_channel_id, &"Update pending ...".to_string())
                .await?;

            bot.create_eve_character(
                refresh_token,
                eve_character_id,
                claims.name,
                discord_character_id,
                discord_guild_id,
                discord_channel_id,
                discord_activity_thread_id,
            )
            .await;
            return Ok(discord_channel_id);
        }

        error!(
            "Failed to decode JWT requested by Discord ID {}.",
            discord_character_id
        );
        return Err("Internal error.".to_string());
    }

    pub async fn has_eve_character(&self, eve_character_id: u64) -> bool {
        let eve_character_list = self.eve_character_list.lock().await;

        for character in eve_character_list.iter() {
            if character.id == eve_character_id {
                return true;
            }
        }

        return false;
    }
}
