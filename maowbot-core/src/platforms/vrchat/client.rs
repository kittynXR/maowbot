// File: maowbot-core/src/platforms/vrchat/client.rs

use reqwest::Client;
use serde::Deserialize;
use crate::Error;
use chrono::{DateTime, Utc};
use tracing::debug;

/// Encapsulates VRChat REST calls that require the user session cookie.
/// For example: fetch current user (to see current world & avatar), fetch world details, change avatar, etc.
pub struct VRChatClient {
    pub session_cookie: String, // e.g. "auth=abcd1234"
    pub http_client: Client,
}

/// Basic info about the user’s current world.
#[derive(Debug)]
pub struct VRChatWorldInfo {
    pub world_id: String,
    pub name: String,
    pub author_name: String,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub capacity: u32,
}

/// Basic info about the user’s current avatar.
#[derive(Debug)]
pub struct VRChatAvatarInfo {
    pub avatar_id: String,
    pub name: String,
}

/// Internal JSON shape for the "currentUser" from `/auth/user?details=...`
#[derive(Debug, Deserialize)]
pub struct VRChatUserJson {
    id: String,
    display_name: String,
    pub(crate) current_avatar: String,
    current_avatar_image_url: Option<String>,
    current_avatar_asset_url: Option<String>,
    current_avatar_unity_package_url: Option<String>,
    pub(crate) current_world: Option<String>, // e.g. "wrld_abcdef12345"
    // more fields if needed...
}

/// JSON shape for "GET /api/1/worlds/{worldId}"
#[derive(Debug, Deserialize)]
struct VRChatWorldJson {
    id: String,
    name: String,
    authorName: String,
    capacity: u32,
    updated_at: String,
    created_at: String,
}

/// JSON shape for "GET /api/1/avatars/{avatarId}"
#[derive(Debug, Deserialize)]
struct VRChatAvatarJson {
    id: String,
    name: String,
    // more fields if needed...
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
    pub async fn fetch_current_user(&self) -> Result<VRChatUserJson, Error> {
        let url = "https://api.vrchat.cloud/api/1/auth/user?details=all";
        let resp = self.http_client
            .get(url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("VRChat fetch_current_user() request failed: {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(format!("VRChat /auth/user failed: HTTP {st} => {txt}")));
        }

        let user_json = resp.json::<VRChatUserJson>().await
            .map_err(|e| Error::Platform(format!("Parsing VRChatUserJson => {e}")))?;
        Ok(user_json)
    }

    /// Fetch info about a given world.
    pub async fn fetch_world_info(&self, world_id: &str) -> Result<VRChatWorldInfo, Error> {
        let url = format!("https://api.vrchat.cloud/api/1/worlds/{}", world_id);
        let resp = self.http_client
            .get(&url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("VRChat fetch_world_info request failed: {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(format!("VRChat GET /worlds/{world_id} => HTTP {st}, {txt}")));
        }

        let wj = resp.json::<VRChatWorldJson>().await
            .map_err(|e| Error::Platform(format!("Parsing VRChatWorldJson => {e}")))?;

        // Convert times:
        let updated_at = parse_vrc_date(&wj.updated_at)?;
        let created_at = parse_vrc_date(&wj.created_at)?;

        let info = VRChatWorldInfo {
            world_id: wj.id,
            name: wj.name,
            author_name: wj.authorName,
            updated_at,
            created_at,
            capacity: wj.capacity,
        };
        Ok(info)
    }

    /// Fetch info about a specific avatar.
    pub async fn fetch_avatar_info(&self, avatar_id: &str) -> Result<VRChatAvatarInfo, Error> {
        let url = format!("https://api.vrchat.cloud/api/1/avatars/{}", avatar_id);
        let resp = self.http_client
            .get(&url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("VRChat fetch_avatar_info request failed: {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(format!("VRChat GET /avatars/{avatar_id} => HTTP {st}, {txt}")));
        }

        let aj = resp.json::<VRChatAvatarJson>().await
            .map_err(|e| Error::Platform(format!("Parsing VRChatAvatarJson => {e}")))?;

        let info = VRChatAvatarInfo {
            avatar_id: aj.id,
            name: aj.name,
        };
        Ok(info)
    }

    /// Change to a new avatar by ID.
    pub async fn select_avatar(&self, avatar_id: &str) -> Result<(), Error> {
        let url = format!("https://api.vrchat.cloud/api/1/avatars/{}/select", avatar_id);
        let resp = self.http_client
            .put(&url)
            .header("Cookie", &self.session_cookie)
            .send()
            .await
            .map_err(|e| Error::Platform(format!("VRChat select_avatar request failed: {e}")))?;

        if !resp.status().is_success() {
            let st = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(Error::Platform(format!("Selecting VRChat avatar {avatar_id} => HTTP {st}, {txt}")));
        }

        debug!("Successfully selected avatar {avatar_id} on VRChat.");
        Ok(())
    }
}

/// The VRChat API often returns timestamps like "2023-02-05T04:19:27.749Z".
/// A quick parse with chrono for RFC3339/ISO8601.
fn parse_vrc_date(s: &str) -> Result<DateTime<Utc>, Error> {
    let dt = s.parse::<DateTime<Utc>>()
        .map_err(|e| Error::Platform(format!("Error parsing VRChat date '{s}': {e}")))?;
    Ok(dt)
}