use serde::{Deserialize, Serialize};

use crate::esi::EsiSkill;

use super::Monitor;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorageV1 {
    pub refresh_token: String,
    pub expired: bool,
    pub eve_character_id: u64,
    pub eve_character_name: String,
    pub discord_character_id: u64,
    pub discord_guild_id: u64,
    pub discord_channel_id: u64,
    pub discord_activity_thread_id: u64,
    pub skills: Vec<EsiSkill>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "version")]
enum Storage {
    #[serde(rename = "1")]
    V1(StorageV1),
}

impl Monitor {
    pub fn read_from_storage(&self, eve_character_id: u64) -> Result<StorageV1, String> {
        let storage_path = format!("{}/char-{}.json", self.storage_folder, eve_character_id);

        let storage = std::fs::read_to_string(&storage_path).map_err(|_| "Character not found.")?;
        let storage: Storage = serde_json::from_str(&storage).map_err(|_| "Invalid storage.")?;
        match storage {
            Storage::V1(storage) => Ok(storage),
        }
    }

    pub fn write_to_storage(&self, eve_character_id: u64, storage: StorageV1) {
        let storage_path = format!("{}/char-{}.json", self.storage_folder, eve_character_id);

        std::fs::write(
            &storage_path,
            serde_json::to_string(&Storage::V1(storage.clone())).unwrap(),
        )
        .unwrap();
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
        let storage = StorageV1 {
            refresh_token,
            expired: false,
            eve_character_id,
            eve_character_name,
            discord_character_id,
            discord_guild_id,
            discord_channel_id,
            discord_activity_thread_id,
            skills: Vec::new(),
        };

        self.write_to_storage(eve_character_id, storage);
        self.queue_eve_character(eve_character_id).await;
    }

    pub async fn refresh_eve_character(
        &self,
        eve_character_id: u64,
        refresh_token: &String,
    ) -> Result<u64, String> {
        let storage = self.read_from_storage(eve_character_id)?;

        if !storage.expired {
            return Err("This character is already monitored.".to_string());
        }

        let storage = StorageV1 {
            refresh_token: refresh_token.clone(),
            expired: false,
            ..storage
        };

        let discord_channel_id = storage.discord_channel_id;

        self.write_to_storage(eve_character_id, storage);
        self.queue_eve_character(eve_character_id).await;
        Ok(discord_channel_id)
    }
}
