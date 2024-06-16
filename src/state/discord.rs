use serenity::all::{
    ChannelId, ChannelType, CreateChannel, CreateThread, EditMessage, GetMessages, Guild, GuildId,
    PermissionOverwrite, PermissionOverwriteType, Permissions, UserId,
};

use super::BotState;

impl BotState {
    pub async fn discord_create_private_channel(
        &self,
        guild_id: u64,
        category_id: u64,
        character_id: u64,
        name: &str,
    ) -> Result<(u64, u64), String> {
        let this = self.0.read().await;
        let http = &this.discord.as_ref().unwrap().http;

        let guild = Guild::get(http, guild_id).await.unwrap();

        let everyone = GuildId::everyone_role(&guild.id);

        /* EVE names can contains spaces or single quotation. Replace them with dashes. */
        let slug = name.replace(' ', "-").replace('\'', "-");

        let permissions_both = Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY;
        let permissions_bot = Permissions::SEND_MESSAGES
            | Permissions::SEND_MESSAGES_IN_THREADS
            | Permissions::CREATE_PUBLIC_THREADS
            | Permissions::CREATE_PRIVATE_THREADS;

        /* Make it a private channel where only the bot can speak, and the user can read. */
        let permissions = vec![
            PermissionOverwrite {
                allow: permissions_both | permissions_bot,
                deny: Permissions::empty(),
                kind: PermissionOverwriteType::Member(UserId::new(1251447020741464116)),
            },
            PermissionOverwrite {
                allow: permissions_both,
                deny: permissions_bot,
                kind: PermissionOverwriteType::Member(UserId::new(character_id)),
            },
            PermissionOverwrite {
                allow: Permissions::empty(),
                deny: Permissions::VIEW_CHANNEL,
                kind: PermissionOverwriteType::Role(everyone),
            },
        ];

        let builder = CreateChannel::new(slug)
            .kind(ChannelType::Text)
            .category(category_id)
            .topic(format!("Skill training status of {}.", name))
            .permissions(permissions);
        let channel = guild.create_channel(http, builder).await.map_err(|e| e.to_string())?;

        let thread = channel
            .create_thread(
                http,
                CreateThread::new("Activity")
                    .auto_archive_duration(serenity::all::AutoArchiveDuration::OneWeek)
                    .kind(ChannelType::PublicThread),
            )
            .await
            .map_err(|e| e.to_string())?;
        thread
            .say(
                http,
                format!(
                    "<@{}>: here I will let you know when a skill training finished.",
                    character_id
                ),
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok((channel.id.get(), thread.id.get()))
    }

    pub async fn discord_send_message(&self, channel_id: u64, message: &String) -> Result<(), String> {
        let this = self.0.read().await;
        let http = &this.discord.as_ref().unwrap().http;

        let channel_id = ChannelId::new(channel_id);
        channel_id.say(http, message).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn discord_edit_last_message(&self, channel_id: u64, message: &String) -> Result<(), String> {
        let this = self.0.read().await;
        let http = &this.discord.as_ref().unwrap().http;

        let channel_id = ChannelId::new(channel_id);
        let mut messages = channel_id
            .messages(http, GetMessages::new().limit(1))
            .await.map_err(|e| e.to_string())?;
        messages[0]
            .edit(http, EditMessage::new().content(message))
            .await.map_err(|e| e.to_string())?;

        Ok(())
    }
}
