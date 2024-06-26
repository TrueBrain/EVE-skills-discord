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
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        _ => "??",
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

    pub async fn update_character(&self, character: &mut Character) -> bool {
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
                    (Ok(mut skills), Ok(skill_queue)) => {
                        character.retries = 0;

                        /* There can we skills in the queue that are in the past. Apply those to the actual skills already. */
                        for queue in &skill_queue.0 {
                            if queue.finish_date.is_none() {
                                continue;
                            }
                            if queue.finish_date.unwrap() > chrono::Utc::now() {
                                break;
                            }

                            let skill = skills
                                .skills
                                .iter_mut()
                                .find(|s| s.skill_id == queue.skill_id);

                            match skill {
                                Some(skill) => {
                                    skill.trained_skill_level = queue.finished_level;
                                }
                                None => {
                                    /* Impossible, as you always have at least the L0 if you are training. */
                                }
                            }
                        }

                        let message = self.skill_queue_to_message(skill_queue).await;
                        let _ = self
                            .bot
                            .discord_edit_last_message(storage.discord_channel_id, &message)
                            .await;

                        /* Don't check for changes if this is our first time loading. */
                        if storage.skills.len() != 0 {
                            let message = self.skills_change(&storage.skills, &skills.skills).await;
                            if !message.is_empty() {
                                let _ = self
                                    .bot
                                    .discord_send_message(
                                        storage.discord_activity_thread_id,
                                        &message,
                                    )
                                    .await;
                            }
                        }

                        storage.skills = skills.skills;
                    }
                    (Err(error), _) => {
                        character.retries += 1;
                        warn!(
                            "[{}] Failed to fetch skills (attempt {} / 8): {}",
                            character.id, character.retries, error
                        );
                    }
                    (_, Err(error)) => {
                        character.retries += 1;
                        warn!(
                            "[{}] Failed to fetch skill queue (attempt {} / 8): {}",
                            character.id, character.retries, error
                        );
                    }
                }
            }
            Err(error) => {
                character.retries += 1;
                warn!(
                    "[{}] Failed to refresh token (attempt {} / 8): {}",
                    character.id, character.retries, error
                );
            }
        }

        let res = if character.retries >= 8 {
            warn!(
                "[{}] Character has failed to load 8 times in a row. Suspending character.",
                character.id
            );

            let _ = self.bot.discord_send_message(
                storage.discord_activity_thread_id,
                &format!("<@{}>: Failed to retrieve Character information eight times in a row. Please re-authenticate with /monitor to continue monitoring. Monitoring suspended.", storage.discord_character_id),
            ).await;

            /* Mark the character as expired and inform our caller we should be removed. */
            storage.expired = true;
            false
        } else {
            true
        };

        self.write_to_storage(character.id, storage.clone());

        res
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

                    if !self.update_character(character).await {
                        let character_id = character.id;
                        list.retain(|c| c.id != character_id);

                        /* Only wrap if we were the last entry; otherwise the current index is the next character. */
                        if *index >= list.len() {
                            *index = 0;
                        }

                        if list.is_empty() {
                            1
                        } else {
                            list.len()
                        }
                    } else {
                        *index = (*index + 1) % list.len();

                        list.len()
                    }
                }
            };

            let sleep_time = tokio::time::Duration::from_secs(30 * 60 / list_len as u64);

            /* Space updating characters evenly over a 30 minute period. */
            let elapsed = now.elapsed();
            if elapsed < sleep_time {
                tokio::time::sleep(sleep_time - elapsed).await;
            }
        }
    }

    async fn skill_queue_to_message(&self, skill_queue: crate::esi::EsiSkillQueue) -> String {
        let mut message = String::new();
        let mut index = 0;
        for queue in &skill_queue.0 {
            /* Only list entries that are stalled or are in the future. */
            if queue.finish_date.is_some() && queue.finish_date <= Some(chrono::Utc::now()) {
                continue;
            }

            let finish_date = match queue.finish_date {
                Some(finish_date) => format!("finish training <t:{}:R>", finish_date.timestamp()),
                None => "never finish training".to_string(),
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
                "- `{} {}` will {}.\n",
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
                Some(finish_date) => format!("finish <t:{}:R>", finish_date.timestamp()),
                None => "never finish".to_string(),
            },
            None => "never finish".to_string(),
        };

        message += &format!("\nSkill queue will {}.\n", finish_date);
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

            match old_skill {
                Some(old_skill) => {
                    if old_skill.trained_skill_level != new_skill.trained_skill_level {
                        let new_skill_name = self
                            .bot
                            .lookup_skill_name(new_skill.skill_id)
                            .await
                            .unwrap_or("Unknown".to_string());

                        message += &format!(
                            "`{} {}` has finished training.\n",
                            new_skill_name,
                            level_to_roman(new_skill.trained_skill_level),
                        );
                    }
                }
                None => {
                    let new_skill_name = self
                        .bot
                        .lookup_skill_name(new_skill.skill_id)
                        .await
                        .unwrap_or("Unknown".to_string());

                    match new_skill.trained_skill_level {
                        0 => {
                            message += &format!("`{}` has been injected.\n", new_skill_name,);
                        }
                        _ => {
                            message += &format!(
                                "`{} {}` has finished training.\n",
                                new_skill_name,
                                level_to_roman(new_skill.trained_skill_level),
                            );
                        }
                    }
                }
            }
        }

        message
    }
}
