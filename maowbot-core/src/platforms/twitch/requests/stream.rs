// ========================================================
// File: maowbot-core/src/platforms/twitch/requests/stream.rs
// ========================================================
use serde::Deserialize;
use tracing::{debug, warn};

use crate::Error;
use crate::platforms::twitch::client::TwitchHelixClient;

/// Response from "Get Streams" endpoint.
#[derive(Debug, Deserialize)]
pub struct StreamsResponse {
    pub data: Vec<StreamData>,
}

/// Single stream data record.
#[derive(Debug, Deserialize)]
pub struct StreamData {
    pub id: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub game_id: String,
    pub game_name: String, // NEW: Added field for game/category name.
    #[serde(rename = "type")]
    pub type_field: String, // e.g., "live"
    pub title: String,
    pub viewer_count: u32,
    pub started_at: String,
    pub language: String,
    pub thumbnail_url: String,
}

/// Response from "Get Users" endpoint.
#[derive(Debug, Deserialize)]
pub struct UsersResponse {
    pub data: Vec<UserData>,
}

/// Single user record.
#[derive(Debug, Deserialize)]
pub struct UserData {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub profile_image_url: String,
}

/// Response from "Get Games" endpoint.
#[derive(Debug, Deserialize)]
pub struct GamesResponse {
    pub data: Vec<GameData>,
}

/// Single game record.
#[derive(Debug, Deserialize)]
pub struct GameData {
    pub id: String,
    pub name: String,
    pub box_art_url: String,
}

/// Struct representing additional Twitch stream details.
#[derive(Debug)]
pub struct StreamDetails {
    pub broadcaster_name: String,
    pub stream_title: String,
    pub game: String,
    pub game_thumbnail: String,
    pub pfp: String,
}

/// Fetches stream details for the given Twitch user by calling Twitch’s Helix endpoints.
///
/// It performs:
///   1. A request to "Get Streams" (to obtain the live stream details).
///   2. A request to "Get Users" (to get the profile image).
///   3. A request to "Get Games" (to resolve the game name and its thumbnail).
pub async fn fetch_stream_details(
    client: &TwitchHelixClient,
    twitch_name: &str,
) -> Result<StreamDetails, Error> {
    // 1. Get stream info
    let streams_url = format!("https://api.twitch.tv/helix/streams?user_login={}", twitch_name);
    let streams_resp = client
        .http_client()
        .get(&streams_url)
        .header("Client-Id", client.client_id())
        .header("Authorization", format!("Bearer {}", client.bearer_token()))
        .send()
        .await
        .map_err(|e| Error::Platform(format!("fetch_stream_details network error: {}", e)))?;

    if !streams_resp.status().is_success() {
        let status = streams_resp.status();
        let body_text = streams_resp.text().await.unwrap_or_default();
        return Err(Error::Platform(format!(
            "fetch_stream_details: HTTP {} => {}",
            status, body_text
        )));
    }

    let streams_body = streams_resp.text().await?;
    let streams_data: StreamsResponse = serde_json::from_str(&streams_body)
        .map_err(|e| Error::Platform(format!("fetch_stream_details parse error: {}", e)))?;

    if streams_data.data.is_empty() {
        return Err(Error::Platform("No live stream found for user".into()));
    }

    let stream = &streams_data.data[0];
    let broadcaster_name = stream.user_name.clone();
    let stream_title = stream.title.clone();
    let game_name_from_stream = stream.game_name.clone();

    // 2. Get user info to obtain profile image (pfp)
    let users_url = format!("https://api.twitch.tv/helix/users?login={}", twitch_name);
    let users_resp = client
        .http_client()
        .get(&users_url)
        .header("Client-Id", client.client_id())
        .header("Authorization", format!("Bearer {}", client.bearer_token()))
        .send()
        .await
        .map_err(|e| Error::Platform(format!("fetch_stream_details user network error: {}", e)))?;

    if !users_resp.status().is_success() {
        let status = users_resp.status();
        let body_text = users_resp.text().await.unwrap_or_default();
        return Err(Error::Platform(format!(
            "fetch_stream_details user: HTTP {} => {}",
            status, body_text
        )));
    }

    let users_body = users_resp.text().await?;
    let users_data: UsersResponse = serde_json::from_str(&users_body)
        .map_err(|e| Error::Platform(format!("fetch_stream_details user parse error: {}", e)))?;

    if users_data.data.is_empty() {
        return Err(Error::Platform("User data not found".into()));
    }

    let user = &users_data.data[0];
    let pfp = user.profile_image_url.clone();

    // 3. Get game details using the game_id from the stream info.
    let game_url = format!("https://api.twitch.tv/helix/games?id={}", stream.game_id);
    let game_resp = client
        .http_client()
        .get(&game_url)
        .header("Client-Id", client.client_id())
        .header("Authorization", format!("Bearer {}", client.bearer_token()))
        .send()
        .await
        .map_err(|e| Error::Platform(format!("fetch_stream_details game network error: {}", e)))?;

    if !game_resp.status().is_success() {
        let status = game_resp.status();
        let body_text = game_resp.text().await.unwrap_or_default();
        return Err(Error::Platform(format!(
            "fetch_stream_details game: HTTP {} => {}",
            status, body_text
        )));
    }

    let game_body = game_resp.text().await?;
    let games_data: GamesResponse = serde_json::from_str(&game_body)
        .map_err(|e| Error::Platform(format!("fetch_stream_details game parse error: {}", e)))?;

    // Use the game data if available; otherwise fall back to what the stream returned.
    let (game_name, game_thumbnail) = if let Some(game) = games_data.data.first() {
        let name = game.name.clone();
        // Use the game's box_art_url as the thumbnail and replace the size placeholders.
        let thumbnail = game.box_art_url.replace("{width}", "285").replace("{height}", "380");
        (name, thumbnail)
    } else if !game_name_from_stream.is_empty() {
        (game_name_from_stream, "".to_string())
    } else {
        ("Unknown Game".to_string(), "".to_string())
    };

    debug!(
        "Fetched stream details: broadcaster='{}', title='{}', game='{}'",
        broadcaster_name, stream_title, game_name
    );

    Ok(StreamDetails {
        broadcaster_name,
        stream_title,
        game: game_name,
        game_thumbnail,
        pfp,
    })
}
