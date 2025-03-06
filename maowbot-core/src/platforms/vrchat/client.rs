use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::Error;
use chrono::{DateTime, Utc, Duration};
use tracing::{info, warn, error};
use tokio::time::sleep;

/// Encapsulates VRChat REST calls that require the user session cookie.
pub struct VRChatClient {
    pub session_cookie: String, // e.g. "auth=abcd1234"
    pub http_client: Client,
}

/// Extended struct for returning “current world.”
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VRChatWorldInfo {
    pub world_id: String,
    pub name: String,
    pub author_name: String,
    pub capacity: u32,

    /// Optional textual description of the world.
    pub description: Option<String>,

    /// The date/time the world was first published.
    pub published_at: Option<String>,

    /// The date/time the world was last updated.
    pub updated_at: Option<String>,

    /// E.g. "public", "private", "hidden", "all ...", or "community labs"
    pub release_status: Option<String>,
}

/// Minimal struct for returning “current avatar.”
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VRChatAvatarInfo {
    pub avatar_id: String,
    pub name: String,
}

/// Minimal struct for returning “current instance.”
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VRChatInstanceInfo {
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub location: Option<String>,
}

/// JSON shape for “GET /users/{userId}”.
#[derive(Debug, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct VRChatUserPublicApiJson {
    id: String,
    display_name: Option<String>,
    world_id: Option<String>,
    instance_id: Option<String>,
    location: Option<String>,
    state: Option<String>,
    status: Option<String>,
    status_description: Option<String>,
}

impl Default for VRChatUserPublicApiJson {
    fn default() -> Self {
        Self {
            id: "".to_string(),
            display_name: None,
            world_id: None,
            instance_id: None,
            location: None,
            state: None,
            status: None,
            status_description: None,
        }
    }
}

/// JSON shape for `GET /auth/user` – used just to get your own userId.
#[derive(Debug, Deserialize)]
#[serde(default)]
struct VRChatAuthUserJson {
    pub id: String,
    pub display_name: String,
    pub current_avatar: String,
    pub current_world: Option<String>,
    pub state: Option<String>,
    pub status: Option<String>,
}

impl Default for VRChatAuthUserJson {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            current_avatar: String::new(),
            current_world: None,
            state: None,
            status: None,
        }
    }
}

/// JSON shape for “GET /worlds/...”
#[derive(Debug, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct VRChatWorldJson {
    pub id: String,
    pub name: String,
    pub author_name: String,
    pub capacity: u32,

    pub description: Option<String>,
    pub publication_date: Option<String>,
    #[serde(rename = "updated_at")]
    pub updated_at: Option<String>,
    pub release_status: Option<String>,
}

impl Default for VRChatWorldJson {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            author_name: String::new(),
            capacity: 0,
            description: None,
            publication_date: None,
            updated_at: None,
            release_status: None,
        }
    }
}

/// JSON shape for “GET /avatars/...”
#[derive(Debug, Deserialize)]
#[serde(default)]
struct VRChatAvatarJson {
    pub id: String,
    pub name: String,
}

impl Default for VRChatAvatarJson {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
        }
    }
}

impl VRChatClient {
    /// Creates a new VRChatClient with the given `session_cookie`.
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

    /// Fetch your own userId from `/auth/user`.
    async fn fetch_current_user_id(&self) -> Result<String, Error> {
        let url = "https://api.vrchat.cloud/api/1/auth/user";
        let resp = self.http_client
            .get(url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("fetch_current_user_id: request failed => {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(
                format!("VRChat /auth/user failed: HTTP {st} => {txt}")
            ));
        }

        let user_json: VRChatAuthUserJson = resp.json().await.map_err(|e| {
            Error::Platform(format!("Parsing VRChatAuthUserJson => {e}"))
        })?;

        if user_json.id.is_empty() {
            Err(Error::Platform("No userId returned by VRChat /auth/user.".to_string()))
        } else {
            Ok(user_json.id)
        }
    }

    /// Fetch a user’s “public” info from `/users/{userId}`, which includes `worldId`, `instanceId`, etc.
    async fn fetch_user_public(&self, user_id: &str) -> Result<VRChatUserPublicApiJson, Error> {
        let url = format!("https://api.vrchat.cloud/api/1/users/{user_id}");
        let resp = self.http_client
            .get(&url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("fetch_user_public: request => {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(
                format!("VRChat GET /users/{user_id} => HTTP {st}, {txt}")
            ));
        }

        let parsed: VRChatUserPublicApiJson = resp.json().await.map_err(|e| {
            Error::Platform(format!("Parsing VRChatUserPublicApiJson => {e}"))
        })?;
        Ok(parsed)
    }

    /// Fetch the user’s current world (up to 3 attempts).
    pub async fn fetch_current_world_api(&self) -> Result<Option<VRChatWorldInfo>, Error> {
        for attempt in 1..=3 {
            info!("Attempt {attempt} to fetch current world from VRChat...");

            let my_user_id = match self.fetch_current_user_id().await {
                Ok(id) => id,
                Err(e) => {
                    error!("Failed to get local userId => {e}");
                    if attempt < 3 {
                        warn!("Will retry in 5 seconds...");
                        sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    return Err(e);
                }
            };

            let public_info = match self.fetch_user_public(&my_user_id).await {
                Ok(info) => info,
                Err(e) => {
                    error!("Failed to fetch /users => {e}");
                    if attempt < 3 {
                        warn!("Will retry in 5 seconds...");
                        sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    return Err(e);
                }
            };

            if let Some(wid) = &public_info.world_id {
                info!("Detected user is in world_id: {wid}");
                match self.fetch_world_info(wid).await {
                    Ok(winfo) => {
                        return Ok(Some(winfo));
                    }
                    Err(e) => {
                        error!("Fetching world details failed => {e:?}");
                        return Err(e);
                    }
                }
            } else {
                warn!("User is offline or no worldId. location={:?}", public_info.location);
                if attempt < 3 {
                    warn!("Will retry in 5 seconds...");
                    sleep(std::time::Duration::from_secs(5)).await;
                } else {
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }

    /// Fetch the user’s current instance (world_id + instance_id) with up to 3 attempts.
    pub async fn fetch_current_instance_api(&self) -> Result<Option<VRChatInstanceInfo>, Error> {
        for attempt in 1..=3 {
            info!("Attempt {attempt} to fetch current instance from VRChat...");

            let my_user_id = match self.fetch_current_user_id().await {
                Ok(id) => id,
                Err(e) => {
                    error!("Failed to get local userId => {e}");
                    if attempt < 3 {
                        warn!("Will retry in 5 seconds...");
                        sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    return Err(e);
                }
            };

            let public_info = match self.fetch_user_public(&my_user_id).await {
                Ok(info) => info,
                Err(e) => {
                    error!("Failed to fetch /users => {e}");
                    if attempt < 3 {
                        warn!("Will retry in 5 seconds...");
                        sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    return Err(e);
                }
            };

            if public_info.location.as_deref() != Some("offline") {
                // user is "online" in some instance
                let inst = VRChatInstanceInfo {
                    world_id: public_info.world_id,
                    instance_id: public_info.instance_id,
                    location: public_info.location,
                };
                // If there's no instance or no world_id, we might keep trying or return None
                if inst.world_id.is_none() && inst.instance_id.is_none() {
                    warn!("User is online but has no valid world/instance. Possibly hidden? Attempt {attempt}...");
                    if attempt < 3 {
                        sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    } else {
                        return Ok(None);
                    }
                }
                return Ok(Some(inst));
            } else {
                warn!("User is offline. Attempt {attempt}...");
                if attempt < 3 {
                    sleep(std::time::Duration::from_secs(5)).await;
                } else {
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }

    /// Fetch the user’s current avatar from `/auth/user?details=all`.
    pub async fn fetch_current_avatar_api(&self) -> Result<Option<VRChatAvatarInfo>, Error> {
        let url = "https://api.vrchat.cloud/api/1/auth/user?details=all";
        let resp = self.http_client
            .get(url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("fetch_current_avatar_api => {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(
                format!("VRChat /auth/user?details=all => HTTP {st}, {txt}")
            ));
        }

        let user_json: VRChatAuthUserJson = resp.json().await.map_err(|e| {
            Error::Platform(format!("Parsing VRChatAuthUserJson => {e}"))
        })?;

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
            author_name: wj.author_name,
            capacity: wj.capacity,
            description: wj.description,
            published_at: wj.publication_date,
            updated_at: wj.updated_at,
            release_status: wj.release_status,
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

    /// Change to a new avatar by ID. (Stub or partial)
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
