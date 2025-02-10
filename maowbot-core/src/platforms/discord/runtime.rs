use std::num::NonZeroU16;
use async_trait::async_trait;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::client::Client;
use serenity::framework::{standard, StandardFramework};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel, UnboundedSender};
use tracing::{info, error};
use crate::Error;
use crate::platforms::{ConnectionStatus, PlatformAuth, PlatformIntegration};

/// The struct that runs a Discord bot via Serenity.
pub struct DiscordPlatform {
    token: String,
    connection_status: ConnectionStatus,
    rx: Option<UnboundedReceiver<DiscordMessageEvent>>,
}

/// The event data we pass along
#[derive(Debug, Clone)]
pub struct DiscordMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub username: String,
    pub text: String,
}

struct Handler {
    tx: UnboundedSender<DiscordMessageEvent>,
}

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        let channel_str = msg.channel_id.to_string();
        let user_id = msg.author.id.to_string();
        let username = msg.author.name.clone();
        let text = msg.content.clone();

        let _ = self.tx.send(DiscordMessageEvent {
            channel: channel_str,
            user_id,
            username,
            text,
        });
    }

    async fn ready(&self, _: Context, data_about_bot: Ready) {
        // If discriminator is None, use 0:
        let disc: u16 = data_about_bot.user.discriminator.map(NonZeroU16::get).unwrap_or(0);
        info!("Discord bot connected as '{}#{}'",
              data_about_bot.user.name, disc);
    }
}

impl DiscordPlatform {
    pub fn new(token: String) -> Self {
        Self {
            token,
            connection_status: ConnectionStatus::Disconnected,
            rx: None,
        }
    }

    /// Wait for the next message event from the internal channel.
    /// Returns `None` if the channel closes.
    pub async fn next_message_event(&mut self) -> Option<DiscordMessageEvent> {
        if let Some(r) = &mut self.rx {
            r.recv().await
        } else {
            None
        }
    }
}

#[async_trait]
impl PlatformAuth for DiscordPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        // For a bot token, there's no separate "authenticate" step, it's just the token.
        Ok(())
    }
    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // no-op
        Ok(())
    }
    async fn revoke_auth(&mut self) -> Result<(), Error> {
        // no-op
        Ok(())
    }
    async fn is_authenticated(&self) -> Result<bool, Error> {
        Ok(!self.token.is_empty())
    }
}

#[async_trait]
impl PlatformIntegration for DiscordPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        // create the unbounded channel
        let (tx, rx) = unbounded_channel::<DiscordMessageEvent>();
        self.rx = Some(rx);
info!("in connect");
        // Build Serenity client with event handler
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;
        let framework = StandardFramework::new();

        let mut client = match Client::builder(&self.token, intents)
            .event_handler(Handler { tx })
            .framework(framework)
            .await
        {
            Ok(c) => c,
            Err(e) => {
                error!("Error creating Serenity client: {:?}", e);
                self.connection_status = ConnectionStatus::Error(format!("{:?}", e));
                return Err(Error::Platform(format!("Serenity init error: {:?}", e)));
            }
        };

        // Start the client in a background task
        tokio::spawn(async move {
            if let Err(e) = client.start_autosharded().await {
                error!("Serenity client error: {:?}", e);
            }
            info!("Serenity client ended.");
        });

        self.connection_status = ConnectionStatus::Connected;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;
        // We cannot forcibly stop Serenity easily except by dropping the client.
        // Our "spawn" is separate, so we might do additional tracking if needed.
        Ok(())
    }

    async fn send_message(&self, _channel: &str, _message: &str) -> Result<(), Error> {
        // For brevity, we skip the actual call: we would parse the channel_id
        // and call e.g. `ChannelId(channel_num).say(&ctx.http, message).await?`
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<crate::platforms::ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}
