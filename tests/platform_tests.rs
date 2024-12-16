// tests/platform_tests.rs

use maowbot::{platforms::*, Error};
use async_trait::async_trait;
use maowbot::platforms::PlatformIntegration;


#[tokio::test]
async fn test_platform_capabilities() -> anyhow::Result<()> {
    #[derive(Debug)]
    struct MockPlatform;

    #[async_trait]
    impl PlatformIntegration for MockPlatform {
        async fn connect(&mut self) -> Result<(), Error> {
            Ok(())
        }
        async fn disconnect(&mut self) -> Result<(), Error> {
            Ok(())
        }
        async fn send_message(&self, _: &str, _: &str) -> Result<(), Error> {
            Ok(())
        }
    }

    // Then implement the other traits
    #[async_trait]
    impl maowbot::platforms::ChatPlatform for MockPlatform {
        async fn join_channel(&self, _: &str) -> Result<(), Error> {
            Ok(())
        }
        async fn leave_channel(&self, _: &str) -> Result<(), Error> {
            Ok(())
        }
        async fn get_channel_users(&self, _: &str) -> Result<Vec<String>, Error> {
            Ok(vec!["user1".to_string(), "user2".to_string()])
        }
    }

    #[async_trait]
    impl maowbot::platforms::StreamingPlatform for MockPlatform {
        async fn get_stream_status(&self, _: &str) -> Result<bool, Error> {
            Ok(true)
        }
        async fn get_viewer_count(&self, _: &str) -> Result<u32, Error> {
            Ok(100)
        }
        async fn update_stream_title(&self, _: &str) -> Result<(), Error> {
            Ok(())
        }
    }

    // Test the platform capabilities
    let mut platform = MockPlatform;
    platform.connect().await?;

    // Test chat functions
    platform.join_channel("test_channel").await?;
    let users = platform.get_channel_users("test_channel").await?;
    assert_eq!(users.len(), 2);

    // Test streaming functions
    assert!(platform.get_stream_status("test_channel").await?);
    assert_eq!(platform.get_viewer_count("test_channel").await?, 100);

    platform.disconnect().await?;

    Ok(())
}

// Keep the original test for basic PlatformIntegration
#[tokio::test]
async fn test_platform_integration() -> anyhow::Result<()> {
    let mut mock = MockPlatformIntegration::new();  // Now this type will be available

    mock.expect_connect()
        .times(1)
        .returning(|| Ok(()));

    mock.expect_send_message()
        .with(mockall::predicate::eq("test_channel"),
              mockall::predicate::eq("test message"))
        .times(1)
        .returning(|_, _| Ok(()));

    mock.expect_disconnect()
        .times(1)
        .returning(|| Ok(()));

    mock.connect().await?;
    mock.send_message("test_channel", "test message").await?;
    mock.disconnect().await?;

    Ok(())
}