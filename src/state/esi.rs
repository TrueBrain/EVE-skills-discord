use crate::esi::{EsiSkillQueue, EsiSkills};

use super::BotState;

impl BotState {
    pub async fn exchange_code(&self, code: String) -> Result<(String, String), String> {
        let this = self.0.read().await;

        this.esi.exchange_code(code).await
    }

    pub async fn exchange_refresh_token(
        &self,
        refresh_token: String,
    ) -> Result<(String, String), String> {
        let this = self.0.read().await;

        this.esi.exchange_refresh_token(refresh_token).await
    }

    pub async fn get_authorization_url(&self, state: String) -> String {
        let this = self.0.read().await;

        this.esi.get_authorization_url(state).await
    }

    pub async fn get_skills(
        &self,
        access_token: &String,
        character_id: u64,
    ) -> Result<EsiSkills, String> {
        let this = self.0.read().await;

        this.esi.get_skills(access_token, character_id).await
    }

    pub async fn get_skill_queue(
        &self,
        access_token: &String,
        character_id: u64,
    ) -> Result<EsiSkillQueue, String> {
        let this = self.0.read().await;

        this.esi.get_skill_queue(access_token, character_id).await
    }

    pub async fn lookup_skill_name(&self, skill_id: i32) -> Result<String, String> {
        let this = self.0.read().await;

        this.esi.lookup_skill_name(skill_id).await
    }
}
