use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde::{Serialize, Deserialize};
use anyhow::Result;
use tracing::{info, error};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
enum BotToPlugin {
    ChatMessage { platform: String, channel: String, user: String, text: String },
    Tick,
    Welcome { bot_name: String },
}

#[derive(Debug, Serialize, Deserialize)]
enum PluginToBot {
    LogMessage { text: String },
    SendChat { channel: String, text: String },
    Hello { plugin_name: String },
    Shutdown,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging via tracing-subscriber
    tracing_subscriber::fmt::init();
    info!("Hello Plugin starting...");

    // Connect to the main bot (adjust IP/port as needed)
    let addr = "127.0.0.1:9999";
    let stream = TcpStream::connect(addr).await?;
    info!("Connected to bot at {}", addr);

    // Split into read and write halves
    let (reader, writer) = stream.into_split();

    // Wrap writer in Arc<Mutex<>> so we can share it across tasks
    let writer = Arc::new(Mutex::new(writer));

    // 1) Immediately send a "Hello { plugin_name }"
    {
        let mut w = writer.lock().await;
        let hello = PluginToBot::Hello {
            plugin_name: "MyHelloPlugin".to_string(),
        };
        let hello_str = serde_json::to_string(&hello)? + "\n";
        w.write_all(hello_str.as_bytes()).await?;
    }

    // 2) Spawn a task to read events from the bot
    let writer_clone = Arc::clone(&writer); // Another reference
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match serde_json::from_str::<BotToPlugin>(&line) {
                Ok(msg) => match msg {
                    BotToPlugin::Welcome { bot_name } => {
                        info!("Bot welcomed us. Bot name: {}", bot_name);
                    }
                    BotToPlugin::ChatMessage { platform, channel, user, text } => {
                        info!("ChatMessage => [{platform}#{channel}] {user}: {text}");

                        // Example: if we see "!ping", respond with "Pong!"
                        if text == "!ping" {
                            let req = PluginToBot::SendChat {
                                channel: channel.clone(),
                                text: "Pong from plugin_hello!".to_string(),
                            };
                            let req_str = serde_json::to_string(&req).unwrap() + "\n";

                            // Write to the writer
                            let mut w = writer_clone.lock().await;
                            if let Err(e) = w.write_all(req_str.as_bytes()).await {
                                error!("Failed to send chat message: {:?}", e);
                            }
                        }
                    }
                    BotToPlugin::Tick => {
                        info!("Received Tick event from the bot!");
                    }
                },
                Err(e) => {
                    error!("Failed to parse message from bot: {} - line was: {}", e, line);
                }
            }
        }

        info!("Plugin read loop ended.");
    });

    // 3) Meanwhile, in the main task, send periodic log messages
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;

        let msg = PluginToBot::LogMessage {
            text: "Hello from plugin_hello, I'm alive!".to_string(),
        };
        let out = serde_json::to_string(&msg)? + "\n";

        let mut w = writer.lock().await;
        w.write_all(out.as_bytes()).await?;
    }
}
