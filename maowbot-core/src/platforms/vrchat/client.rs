// File: maowbot-core/src/platforms/vrchat/client.rs

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value};
use crate::Error;
use chrono::{DateTime, Utc, Duration};
use tracing::{info, warn, error};
use tokio::time::sleep;

/// Encapsulates VRChat REST calls that require the user session cookie.
/// For example: fetch current user, fetch world details, fetch avatar, etc.
pub struct VRChatClient {
    pub session_cookie: String, // e.g. "auth=abcd1234"
    pub http_client: Client,
}

/// Minimal struct for returning “current world.”
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VRChatWorldInfo {
    pub world_id: String,
    pub name: String,
    pub author_name: String,
    pub capacity: u32,
    // You might keep updated_at / created_at if you like
    // pub updated_at: DateTime<Utc>,
    // pub created_at: DateTime<Utc>,
}

/// Minimal struct for returning “current avatar.”
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VRChatAvatarInfo {
    pub avatar_id: String,
    pub name: String,
}

/// Internal JSON shape for the “current user” from `/auth/user?details=all`.
#[derive(Debug, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct VRChatUserJson {
    pub id: String,
    pub display_name: String,
    pub current_avatar: String,
    pub current_world: Option<String>,

    // If VRChat is returning "state", "status", or "requiresTwoFactorAuth", put them here:
    pub state: Option<String>,
    pub status: Option<String>,
    pub requires_two_factor_auth: Option<Vec<String>>,
    // etc.
}

impl Default for VRChatUserJson {
    fn default() -> Self {
        Self {
            id: "".into(),
            display_name: "".into(),
            current_avatar: "".into(),
            current_world: None,
            state: None,
            status: None,
            requires_two_factor_auth: None,
        }
    }
}

/// JSON shape for “GET /api/1/worlds/{worldId}”.
#[derive(Debug, Deserialize)]
struct VRChatWorldJson {
    pub id: String,
    pub name: String,
    pub authorName: String,
    pub capacity: u32,
    // updated_at, created_at, etc. if needed
}

/// JSON shape for “GET /api/1/avatars/{avatarId}”.
#[derive(Debug, Deserialize)]
struct VRChatAvatarJson {
    pub id: String,
    pub name: String,
}

impl VRChatClient {
    /// Creates a new VRChatClient with the given `session_cookie` (e.g. "auth=XXXXX").
    pub fn new(session_cookie: &str) -> Result<Self, Error> {
        let client = reqwest::ClientBuilder::new()
            .user_agent("MaowBot/1.0 cat@kittyn.cat")
            .build()
            .map_err(|e| Error::Platform(format!("Failed to build reqwest client: {e}")))?;

        Ok(Self {
            session_cookie: session_cookie.to_string(),
            http_client: client,
        })
    }

    /// Fetch data about the logged-in user, including `current_world` and `current_avatar`.
    /// (Called internally by our retry methods below.)
    pub async fn fetch_current_user(&self) -> Result<VRChatUserJson, Error> {
        let url = "https://api.vrchat.cloud/api/1/auth/user?details=all";
        let resp = self.http_client
            .get(url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("fetch_current_user: request failed => {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(
                format!("VRChat /auth/user failed: HTTP {st} => {txt}")
            ));
        }

        let text_body = resp
            .text()
            .await
            .map_err(|e| Error::Platform(format!("Unable to read VRChatUserJson body => {e}")))?;

        // Attempt to parse JSON
        let user_json = match serde_json::from_str::<VRChatUserJson>(&text_body) {
            Ok(u) => u,
            Err(e) => {
                // Log it (in production you might want to keep it at `debug!`)
                error!("Raw VRChatUserJson body was: {}", text_body);
                return Err(Error::Platform(format!("Parsing VRChatUserJson => {e}")));
            }
        };

        Ok(user_json)
    }


    /// **New**: fetch current world with up to 3 attempts, sleeping if offline or 401, etc.
    /// Returns Ok(Some(world)) if the user is in a world, Ok(None) if offline or no world found.
    pub async fn fetch_current_world_api(&self) -> Result<Option<VRChatWorldInfo>, Error> {
        for attempt in 1..=3 {
            info!("Attempt {attempt} to fetch current world from VRChat...");

            let user_res = self.fetch_current_user().await;
            match user_res {
                Ok(usr) => {
                    // Check if “current_world” is set
                    if let Some(world_id) = usr.current_world {
                        info!("Detected user is in world_id: {world_id}");
                        // Now fetch that world’s details
                        match self.fetch_world_info(&world_id).await {
                            Ok(winfo) => {
                                return Ok(Some(winfo));
                            }
                            Err(e) => {
                                error!("Fetching world details failed => {e:?}");
                                return Err(e);
                            }
                        }
                    } else {
                        warn!("User is offline or has no current_world. Will retry if attempt < 3...");
                        if attempt < 3 {
                            sleep(std::time::Duration::from_secs(5)).await;
                            continue;
                        } else {
                            return Ok(None);
                        }
                    }
                }
                Err(e) => {
                    // Possibly 401 or something else
                    error!("Failed to fetch current user => {e:?}");
                    if attempt < 3 {
                        warn!("Will retry in 5 seconds...");
                        sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        // We exhausted 3 attempts
        Ok(None)
    }

    /// **New**: fetch current avatar with a simple approach (1 attempt).
    /// You could also do a 3‐attempt approach if you like. Adjust as needed.
    pub async fn fetch_current_avatar_api(&self) -> Result<Option<VRChatAvatarInfo>, Error> {
        let user_json = self.fetch_current_user().await?;
        if user_json.current_avatar.is_empty() {
            return Ok(None);
        }
        let av = self.fetch_avatar_info(&user_json.current_avatar).await?;
        Ok(Some(av))
    }

    /// Helper to fetch world info for a given world_id
    pub async fn fetch_world_info(&self, world_id: &str) -> Result<VRChatWorldInfo, Error> {
        let url = format!("https://api.vrchat.cloud/api/1/worlds/{world_id}");
        let resp = self.http_client
            .get(&url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("VRChat fetch_world_info() request failed: {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(
                format!("VRChat GET /worlds/{world_id} => HTTP {st}, {txt}")
            ));
        }

        let wj = resp.json::<VRChatWorldJson>().await
            .map_err(|e| Error::Platform(format!("Parsing VRChatWorldJson => {e}")))?;

        Ok(VRChatWorldInfo {
            world_id: wj.id,
            name: wj.name,
            author_name: wj.authorName,
            capacity: wj.capacity,
        })
    }

    /// Helper to fetch avatar info for a given avatar_id
    pub async fn fetch_avatar_info(&self, avatar_id: &str) -> Result<VRChatAvatarInfo, Error> {
        let url = format!("https://api.vrchat.cloud/api/1/avatars/{avatar_id}");
        let resp = self.http_client
            .get(&url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("VRChat fetch_avatar_info() request failed: {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(
                format!("VRChat GET /avatars/{avatar_id} => HTTP {st}, {txt}")
            ));
        }

        let aj = resp.json::<VRChatAvatarJson>().await
            .map_err(|e| Error::Platform(format!("Parsing VRChatAvatarJson => {e}")))?;

        Ok(VRChatAvatarInfo {
            avatar_id: aj.id,
            name: aj.name,
        })
    }

    /// Change to a new avatar by ID.
    pub async fn select_avatar(&self, avatar_id: &str) -> Result<(), Error> {
        let url = format!("https://api.vrchat.cloud/api/1/avatars/{avatar_id}/select");
        let resp = self.http_client
            .put(&url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("VRChat select_avatar request failed: {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(
                format!("Selecting VRChat avatar {avatar_id} => HTTP {st}, {txt}")
            ));
        }

        info!("Successfully selected avatar {avatar_id} on VRChat.");
        Ok(())
    }
}