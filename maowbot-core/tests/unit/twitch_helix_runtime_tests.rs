// tests/unit/twitch_helix_runtime_tests.rs

use maowbot_core::platforms::twitch_helix::runtime::{TwitchPlatform, TwitchMessageEvent};
use maowbot_core::platforms::{PlatformIntegration, PlatformAuth, ConnectionStatus};
use maowbot_core::Error;

#[tokio::test]
async fn test_twitch_connect_disconnect() -> Result<(), Error> {
    let mut twitch = TwitchPlatform::new();

    let status = twitch.get_connection_status().await?;
    assert_eq!(status, ConnectionStatus::Disconnected);

    twitch.connect().await?;
    let status = twitch.get_connection_status().await?;
    assert_eq!(status, ConnectionStatus::Connected);

    twitch.disconnect().await?;
    let status = twitch.get_connection_status().await?;
    assert_eq!(status, ConnectionStatus::Disconnected);

    Ok(())
}

#[tokio::test]
async fn test_twitch_auth_flow() -> Result<(), Error> {
    let mut twitch = TwitchPlatform::new();

    let is_auth = twitch.is_authenticated().await?;
    assert!(!is_auth);

    twitch.authenticate().await?;
    let is_auth = twitch.is_authenticated().await?;
    assert!(!is_auth, "We didn't store real credentials, so it remains false in this stub");

    twitch.revoke_auth().await?;
    Ok(())
}

#[tokio::test]
async fn test_twitch_send_message_stub() -> Result<(), Error> {
    let mut twitch = TwitchPlatform::new();
    twitch.connect().await?;

    twitch.send_message("some_channel", "Hello world").await?;
    let status = twitch.get_connection_status().await?;
    assert_eq!(status, ConnectionStatus::Connected);

    Ok(())
}

#[tokio::test]
async fn test_twitch_next_message_event_stub() -> Result<(), Error> {
    let mut twitch = TwitchPlatform::new();
    let evt = twitch.next_message_event().await;
    assert!(evt.is_none(), "Stub returns None");
    Ok(())
}
