// tests/platform_tests.rs
use maowbot::{platforms::*, Error, };
use mockall::automock;
use async_trait::async_trait;

// Create mock platform for testing
#[automock]
#[async_trait]
pub trait TestPlatform: PlatformIntegration {
    async fn test_specific_method(&self) -> Result<String, Error>;
}

#[tokio::test]
async fn test_platform_integration() -> anyhow::Result<()> {
    let mut mock = MockTestPlatform::new();

    // Set up expectations
    mock.expect_connect()
        .times(1)
        .returning(|| Ok(()));

    mock.expect_send_message()
        .with(mockall::predicate::eq("test_channel"), mockall::predicate::eq("test message"))
        .times(1)
        .returning(|_, _| Ok(()));

    mock.expect_disconnect()
        .times(1)
        .returning(|| Ok(()));

    // Test the mock
    mock.connect().await?;
    mock.send_message("test_channel", "test message").await?;
    mock.disconnect().await?;

    Ok(())
}

// Example test for ChatPlatform trait
#[tokio::test]
async fn test_chat_platform() -> anyhow::Result<()> {
    struct MockChatPlatform;

    #[async_trait]
    impl PlatformIntegration for MockChatPlatform {
        async fn connect(&mut self) -> Result<(), Error> { Ok(()) }
        async fn disconnect(&mut self) -> Result<(), Error> { Ok(()) }
        async fn send_message(&self, _: &str, _: &str) -> Result<(), Error> { Ok(()) }
    }

    #[async_trait]
    impl ChatPlatform for MockChatPlatform {
        async fn join_channel(&self, _: &str) -> Result<(), Error> { Ok(()) }
        async fn leave_channel(&self, _: &str) -> Result<(), Error> { Ok(()) }
        async fn get_channel_users(&self, _: &str) -> Result<Vec<String>, Error> {
            Ok(vec!["user1".to_string(), "user2".to_string()])
        }
    }

    let mut platform = MockChatPlatform;
    platform.connect().await?;

    let users = platform.get_channel_users("test_channel").await?;
    assert_eq!(users.len(), 2);

    platform.disconnect().await?;

    Ok(())
}

// Test for StreamingPlatform trait
#[tokio::test]
async fn test_streaming_platform() -> anyhow::Result<()> {
    struct MockStreamingPlatform;

    #[async_trait]
    impl PlatformIntegration for MockStreamingPlatform {
        async fn connect(&mut self) -> Result<(), Error> { Ok(()) }
        async fn disconnect(&mut self) -> Result<(), Error> { Ok(()) }
        async fn send_message(&self, _: &str, _: &str) -> Result<(), Error> { Ok(()) }
    }

    #[async_trait]
    impl StreamingPlatform for MockStreamingPlatform {
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

    let mut platform = MockStreamingPlatform;
    platform.connect().await?;

    assert!(platform.get_stream_status("test_channel").await?);
    assert_eq!(platform.get_viewer_count("test_channel").await?, 100);

    platform.disconnect().await?;

    Ok(())
}