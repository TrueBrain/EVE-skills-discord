use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{atomic::AtomicBool, Arc};

use serenity::all::{CommandInteraction, Context, EditInteractionResponse};
use tokio::sync::RwLock;

use crate::esi::Esi;
use crate::monitor::Monitor;

mod discord;
mod esi;
mod monitor;
mod pending;

struct PendingState {
    expire_time: std::time::Instant,
    interaction: CommandInteraction,
}

struct BotStorage {
    startup: AtomicBool,
    pending: HashMap<String, PendingState>,
    esi: Esi,
    discord: Option<Context>,
    monitor: Option<Arc<Monitor>>,
}

pub struct BotState(Arc<RwLock<BotStorage>>);

impl Clone for BotState {
    fn clone(&self) -> Self {
        BotState(self.0.clone())
    }
}

impl BotState {
    pub fn new(timeout_message: String) -> Self {
        let store = BotState(Arc::new(RwLock::new(BotStorage {
            startup: AtomicBool::new(true),
            pending: HashMap::new(),
            esi: Esi::new(),
            discord: None,
            monitor: None,
        })));

        /* Monitor the pending lists for any expired entry. */
        let store_clone = store.0.clone();
        tokio::spawn(async move {
            loop {
                {
                    let mut store_clone = store_clone.write().await;
                    let now = std::time::Instant::now();

                    let keys: Vec<String> = store_clone.pending.keys().cloned().collect();
                    for key in keys {
                        let value = store_clone.pending.get(&key).unwrap();
                        if value.expire_time > now {
                            continue;
                        }

                        let _ = value
                            .interaction
                            .edit_response(
                                &store_clone.discord.as_ref().unwrap().http,
                                EditInteractionResponse::new().content(&timeout_message),
                            )
                            .await;
                        store_clone.pending.remove(&key);
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        });

        store
    }

    pub async fn set_discord(&self, ctx: Context) {
        let mut this = self.0.write().await;

        this.discord = Some(ctx);
    }

    pub async fn set_monitor(&self, monitor: Arc<Monitor>) {
        let mut this = self.0.write().await;

        this.monitor = Some(monitor);
    }

    pub async fn set_if_startup(&self) -> bool {
        let this = self.0.read().await;

        if !this.startup.load(Ordering::Relaxed) {
            return false;
        }
        this.startup.swap(false, Ordering::Relaxed);

        true
    }
}
