// tests/twitch_runtime_tests.rs

use maowbot::platforms::twitch::runtime::{
    TwitchPlatform, TwitchMessageEvent
};
use maowbot::platforms::{PlatformIntegration, PlatformAuth, ConnectionStatus};
use maowbot::Error;

#[tokio::test]
async fn test_twitch_connect_disconnect() -> Result<(), Error> {
    let mut twitch = TwitchPlatform::new();

    // Initially disconnected
    let status = twitch.get_connection_status().await?;
    assert_eq!(status, ConnectionStatus::Disconnected);

    // Connect
    twitch.connect().await?;
    let status = twitch.get_connection_status().await?;
    assert_eq!(status, ConnectionStatus::Connected);

    // Disconnect
    twitch.disconnect().await?;
    let status = twitch.get_connection_status().await?;
    assert_eq!(status, ConnectionStatus::Disconnected);

    Ok(())
}

#[tokio::test]
async fn test_twitch_auth_flow() -> Result<(), Error> {
    let mut twitch = TwitchPlatform::new();

    // Not authenticated yet
    let is_auth = twitch.is_authenticated().await?;
    assert!(!is_auth, "Should not be authenticated initially");

    // Example: set credentials or call .authenticate()
    twitch.authenticate().await?;
    // For this simple test, we do not actually set credentials,
    // so it's still not “auth” in practice.
    // In real code, you might store them or mock it.

    let is_auth = twitch.is_authenticated().await?;
    assert!(!is_auth, "We didn't store credentials, so it's still false");

    // Revoke to confirm it doesn't panic
    twitch.revoke_auth().await?;
    Ok(())
}

#[tokio::test]
async fn test_twitch_send_message_stub() -> Result<(), Error> {
    let mut twitch = TwitchPlatform::new();
    twitch.connect().await?;

    // Now we await the async call before applying the `?`
    twitch.send_message("some_channel", "Hello world").await?;

    let status = twitch.get_connection_status().await?;
    assert_eq!(status, ConnectionStatus::Connected);

    Ok(())
}


#[tokio::test]
async fn test_twitch_next_message_event_stub() -> Result<(), Error> {
    let mut twitch = TwitchPlatform::new();
    // By default, `next_message_event()` returns None
    let evt = twitch.next_message_event().await;
    assert!(evt.is_none(), "Stub returns None");
    Ok(())
}
