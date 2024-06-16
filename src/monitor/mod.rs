use std::{env, sync::Arc};

use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{esi::EsiSkill, state::BotState};

mod install;
mod load;
mod storage;

pub struct Character {
    id: u64,
    retries: u64,
}

pub struct Monitor {
    bot: BotState,
    storage_folder: String,
    eve_character_list: Arc<Mutex<Vec<Character>>>,
    current_index: Arc<Mutex<usize>>,
}

fn level_to_roman(level: i32) -> &'static str {
    match level {
        0 => "I",
        1 => "II",
        2 => "III",
        3 => "IV",
        4 => "V",
        _ => "VI",
    }
}

impl Monitor {
    pub fn new(bot: BotState) -> Self {
        let storage_folder =
            env::var("STORAGE_FOLDER").expect("Expected STORAGE_FOLDER in the environment");

        Self {
            bot,
            storage_folder,
            eve_character_list: Arc::new(Mutex::new(Vec::new())),
            current_index: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn start(bot: BotState) {
        let monitor = Arc::new(Monitor::new(bot.clone()));
        bot.set_monitor(monitor.clone()).await;
        tokio::spawn(async move {
            monitor.load_all_eve_characters().await;
            monitor.run().await;
        });
    }

    pub async fn update_character(&self, character: &mut Character) {
        info!("[{}] Refreshing skills", character.id);

        let mut storage = self.read_from_storage(character.id).unwrap();

        let tokens = self
            .bot
            .exchange_refresh_token(storage.refresh_token.clone())
            .await;

        match tokens {
            Ok((access_token, new_refresh_token)) => {
                storage.refresh_token = new_refresh_token;

                let skills = self.bot.get_skills(&access_token, character.id).await;
                let skill_queue = self.bot.get_skill_queue(&access_token, character.id).await;

                match (skills, skill_queue) {
                    (Ok(skills), Ok(skill_queue)) => {
                        character.retries = 0;

                        let message = self.skill_queue_to_message(skill_queue).await;
                        let _ = self.bot
                            .discord_edit_last_message(storage.discord_channel_id, &message)
                            .await;

                        /* Don't check for changes if this is our first time loading. */
                        if storage.skills.len() != 0 {
                            let message = self.skills_change(&storage.skills, &skills.skills).await;
                            if !message.is_empty() {
                                let _ = self.bot
                                    .discord_send_message(storage.discord_activity_thread_id, &message)
                                    .await;
                            }
                        }

                        storage.skills = skills.skills;
                    }
                    (Err(error), _) => {
                        warn!("[{}] Failed to fetch skills: {}", character.id, error);
                        character.retries += 1;
                    }
                    (_, Err(error)) => {
                        warn!("[{}] Failed to fetch skill queue: {}", character.id, error);
                        character.retries += 1;
                    }
                }
            }
            Err(error) => {
                warn!("[{}] Failed to refresh token: {}", character.id, error);
                character.retries += 1;
                return;
            }
        }

        if character.retries >= 3 {
            warn!(
                "[{}] Character has failed to refresh 3 times. Suspending account.",
                character.id
            );
            storage.expired = true;

            let _ = self.bot.discord_send_message(
                storage.discord_activity_thread_id,
                &format!("<@{}>: Failed to retrieve Character information three times in a row. Please re-authenticate with /monitor to continue monitoring. Monitoring suspended.", storage.discord_character_id),
            ).await;
        }

        self.write_to_storage(character.id, storage.clone());
    }

    pub async fn run(&self) {
        info!("Starting skill monitor thread");

        loop {
            let now = tokio::time::Instant::now();

            let list_len = {
                let mut list = self.eve_character_list.lock().await;
                let mut index = self.current_index.lock().await;

                if list.is_empty() {
                    1
                } else {
                    let character = &mut list[*index];

                    self.update_character(character).await;

                    *index = (*index + 1) % list.len();
                    list.len()
                }
            };

            /* Space updating characters evenly over a 30 minute period. */
            let elapsed = now.elapsed();
            tokio::time::sleep(
                tokio::time::Duration::from_secs(30 * 60 / list_len as u64) - elapsed,
            )
            .await;
        }
    }

    async fn skill_queue_to_message(&self, skill_queue: crate::esi::EsiSkillQueue) -> String {
        let mut message = String::new();
        let mut index = 0;
        for queue in &skill_queue.0 {
            let finish_date = match queue.finish_date {
                Some(finish_date) => format!("<t:{}:R>", finish_date.timestamp()),
                None => "never".to_string(),
            };
            let skill_name = self.bot.lookup_skill_name(queue.skill_id).await;
            let skill_name = match skill_name {
                Ok(skill_name) => skill_name,
                Err(error) => {
                    warn!(
                        "Failed to lookup skill name for skill ID {}: {}",
                        queue.skill_id, error
                    );
                    "Unknown".to_string()
                }
            };

            message += &format!(
                "- `{} {}` will finish training {}.\n",
                skill_name,
                level_to_roman(queue.finished_level),
                finish_date,
            );
            index += 1;
            if index >= 5 {
                message += &format!("... and {} more.\n", skill_queue.0.len() - 5);
                break;
            }
        }

        /* Find the last skill in queue. */
        let finish_date = match skill_queue.0.last() {
            Some(queue) => match queue.finish_date {
                Some(finish_date) => format!("<t:{}:R>", finish_date.timestamp()),
                None => "never".to_string(),
            },
            None => "never".to_string(),
        };

        message += &format!("\nSkill queue will finish {}.\n", finish_date);
        message += &format!(
            "\nNext update expected <t:{}:R>.\n",
            chrono::Utc::now().timestamp() + 30 * 60
        );

        message
    }

    async fn skills_change(
        &self,
        old_skills: &Vec<EsiSkill>,
        new_skills: &Vec<EsiSkill>,
    ) -> String {
        let mut message = String::new();

        for new_skill in new_skills {
            let old_skill = old_skills
                .iter()
                .find(|&s| s.skill_id == new_skill.skill_id);

            let new_skill_name = self
                .bot
                .lookup_skill_name(new_skill.skill_id)
                .await
                .unwrap_or("Unknown".to_string());

            match old_skill {
                Some(old_skill) => {
                    if old_skill.active_skill_level != new_skill.active_skill_level {
                        message += &format!(
                            "`{} {}` has finished training.\n",
                            new_skill_name,
                            level_to_roman(new_skill.active_skill_level),
                        );
                    }
                }
                None => match new_skill.active_skill_level {
                    0 => {
                        message += &format!("`{}` has been injected.\n", new_skill_name,);
                    }
                    _ => {
                        message += &format!(
                            "`{} {}` has finished training.\n",
                            new_skill_name,
                            level_to_roman(new_skill.active_skill_level),
                        );
                    }
                },
            }
        }

        message
    }
}
