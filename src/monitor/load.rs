use super::{Character, Monitor};

impl Monitor {
    pub async fn load_all_eve_characters(&self) {
        let storage_files = std::fs::read_dir(&self.storage_folder).unwrap();

        for storage_file in storage_files {
            let storage_file = storage_file.unwrap();
            let storage_file = storage_file.path();
            let storage_file = storage_file.file_name().unwrap();

            /* Characters are stored in char-<number>.json. */
            if let Some(storage_file) = storage_file.to_str() {
                if storage_file.starts_with("char-") && storage_file.ends_with(".json") {
                    let eve_character_id: u64 =
                        storage_file[5..storage_file.len() - 5].parse().unwrap();

                    let storage = self.read_from_storage(eve_character_id).unwrap();
                    if !storage.expired {
                        let mut list = self.eve_character_list.lock().await;
                        list.push(Character {
                            id: eve_character_id,
                            retries: 0,
                        });
                    }
                }
            }
        }
    }

    pub async fn queue_eve_character(&self, eve_character_id: u64) {
        let mut character = Character {
            id: eve_character_id,
            retries: 0,
        };

        /* As this is a new entry, update the character immediately. */
        self.update_character(&mut character).await;

        let mut list = self.eve_character_list.lock().await;
        list.push(character);
    }
}
