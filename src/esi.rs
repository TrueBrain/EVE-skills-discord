use std::env;
use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

pub struct Esi {
    oauth: BasicClient,
    skill_name_cache: Arc<Mutex<HashMap<i32, String>>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EsiSkill {
    pub skill_id: i32,
    pub skillpoints_in_skill: i32,
    pub trained_skill_level: i32,
    pub active_skill_level: i32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EsiSkills {
    pub skills: Vec<EsiSkill>,
    pub total_sp: i64,
    pub unallocated_sp: i32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EsiSkillQueueItem {
    pub finish_date: Option<chrono::DateTime<Utc>>,
    pub finished_level: i32,
    pub level_end_sp: i32,
    pub level_start_sp: i32,
    pub queue_position: i32,
    pub skill_id: i32,
    pub start_date: Option<chrono::DateTime<Utc>>,
    pub training_start_sp: i32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct EsiNamesLookup {
    id: u64,
    category: String,
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct EsiSkillQueue(pub Vec<EsiSkillQueueItem>);

impl Esi {
    pub fn new() -> Self {
        let client_id =
            env::var("EVE_CLIENT_ID").expect("Expected EVE_CLIENT_ID in the environment");
        let client_secret =
            env::var("EVE_CLIENT_SECRET").expect("Expected EVE_CLIENT_SECRET in the environment");
        let webserver_url =
            env::var("WEBSERVER_URL").expect("Expected WEBSERVER_URL in the environment");

        let oauth_client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new("https://login.eveonline.com/v2/oauth/authorize".to_string())
                .expect("Invalid authorization URL"),
            Some(
                TokenUrl::new("https://login.eveonline.com/v2/oauth/token".to_string())
                    .expect("Invalid token URL"),
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new(format!("{}/callback", webserver_url)).expect("Invalid redirect URL"),
        );

        Esi {
            oauth: oauth_client,
            skill_name_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn exchange_code(&self, code: String) -> Result<(String, String), String> {
        let token = self
            .oauth
            .exchange_code(AuthorizationCode::new(code))
            .request_async(async_http_client)
            .await;

        if let Ok(token) = token {
            Ok((
                token.access_token().secret().to_string(),
                token.refresh_token().unwrap().secret().to_string(),
            ))
        } else {
            Err("Failed to exchange code".to_string())
        }
    }

    pub async fn exchange_refresh_token(
        &self,
        refresh_token: String,
    ) -> Result<(String, String), String> {
        let token = self
            .oauth
            .exchange_refresh_token(&RefreshToken::new(refresh_token))
            .request_async(async_http_client)
            .await;

        if let Ok(token) = token {
            Ok((
                token.access_token().secret().to_string(),
                token.refresh_token().unwrap().secret().to_string(),
            ))
        } else {
            Err("Failed to exchange refresh token".to_string())
        }
    }

    pub async fn get_authorization_url(&self, state: String) -> String {
        let state = CsrfToken::new(state);

        let (auth_url, _) = self
            .oauth
            .authorize_url(|| state)
            .add_scope(Scope::new("esi-skills.read_skills.v1".to_string()))
            .add_scope(Scope::new("esi-skills.read_skillqueue.v1".to_string()))
            .url();

        auth_url.to_string()
    }

    pub async fn get_skills(
        &self,
        access_token: &String,
        character_id: u64,
    ) -> Result<EsiSkills, String> {
        let url = format!(
            "https://esi.evetech.net/v4/characters/{}/skills/",
            character_id
        );

        let response = reqwest::Client::new()
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await;

        match response {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response.text().await;

                match (status, body) {
                    (200, Ok(body)) => serde_json::from_str(&body).map_err(|e| e.to_string()),
                    (_, Err(body)) => Err(body.to_string()),
                    (status, _) => Err(format!("Failed to fetch skills: status code {}", status)),
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn get_skill_queue(
        &self,
        access_token: &String,
        character_id: u64,
    ) -> Result<EsiSkillQueue, String> {
        let url = format!(
            "https://esi.evetech.net/v2/characters/{}/skillqueue/",
            character_id
        );

        let response = reqwest::Client::new()
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await;

        match response {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response.text().await;

                match (status, body) {
                    (200, Ok(body)) => serde_json::from_str(&body).map_err(|e| e.to_string()),
                    (_, Err(body)) => Err(body.to_string()),
                    (status, _) => Err(format!(
                        "Failed to fetch skill queue: status code {}",
                        status
                    )),
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn lookup_skill_name(&self, skill_id: i32) -> Result<String, String> {
        let mut skill_name_cache = self.skill_name_cache.lock().await;

        if let Some(name) = skill_name_cache.get(&skill_id) {
            return Ok(name.clone());
        }

        let url = format!("https://esi.evetech.net/v3/universe/names/");

        let response = reqwest::Client::new()
            .post(&url)
            .body(format!("[{}]", skill_id))
            .header("Content-Type", "application/json")
            .send()
            .await;

        match response {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response.text().await;

                match (status, body) {
                    (200, Ok(body)) => {
                        let lookups: Vec<EsiNamesLookup> =
                            serde_json::from_str(&body).map_err(|e| e.to_string())?;
                        let lookup = &lookups[0];

                        skill_name_cache.insert(skill_id, lookup.name.clone());

                        Ok(lookup.name.clone())
                    }
                    (_, Err(body)) => Err(body.to_string()),
                    (status, _) => Err(format!(
                        "Failed to fetch skill name: status code {}",
                        status
                    )),
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }
}
