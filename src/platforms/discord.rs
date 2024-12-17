// src/platforms/discord.rs
use serenity::Client;
use crate::models::PlatformCredential;
use crate::platforms::ConnectionStatus;

pub struct DiscordPlatform {
    client: Option<Client>,
    credentials: Option<PlatformCredential>,
    connection_status: ConnectionStatus,
}

// Implement traits similar to Twitch