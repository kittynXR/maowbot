// tests/platform_tests.rs

use maowbot::{platforms::*, Error};
use async_trait::async_trait;

#[tokio::test]
async fn test_platform_capabilities() -> Result<(), Error> {
    #[derive(Debug)]
    struct MockPlatform {
        connection_status: ConnectionStatus,
    }

    impl MockPlatform {
        fn new() -> Self {
            Self {
                connection_status: ConnectionStatus::Disconnected,
            }
        }
    }

    // Implement PlatformAuth
    #[async_trait]
    impl PlatformAuth for MockPlatform {
        async fn authenticate(&mut self) -> Result<(), Error> {
            Ok(())
        }

        async fn refresh_auth(&mut self) -> Result<(), Error> {
            Ok(())
        }

        async fn revoke_auth(&mut self) -> Result<(), Error> {
            Ok(())
        }

        async fn is_authenticated(&self) -> Result<bool, Error> {
            Ok(true)
        }
    }

    // Implement PlatformIntegration
    #[async_trait]
    impl PlatformIntegration for MockPlatform {
        async fn connect(&mut self) -> Result<(), Error> {
            self.connection_status = ConnectionStatus::Connected;
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), Error> {
            self.connection_status = ConnectionStatus::Disconnected;
            Ok(())
        }

        async fn send_message(&self, _channel: &str, _message: &str) -> Result<(), Error> {
            Ok(())
        }

        async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
            Ok(self.connection_status.clone())
        }
    }

    // Implement ChatPlatform
    #[async_trait]
    impl ChatPlatform for MockPlatform {
        async fn join_channel(&self, _channel: &str) -> Result<(), Error> {
            Ok(())
        }

        async fn leave_channel(&self, _channel: &str) -> Result<(), Error> {
            Ok(())
        }

        async fn get_channel_users(&self, _channel: &str) -> Result<Vec<String>, Error> {
            Ok(vec!["user1".to_string(), "user2".to_string()])
        }
    }

    // Implement StreamingPlatform
    #[async_trait]
    impl StreamingPlatform for MockPlatform {
        async fn get_stream_status(&self, _channel: &str) -> Result<bool, Error> {
            Ok(true)
        }

        async fn get_viewer_count(&self, _channel: &str) -> Result<u32, Error> {
            Ok(100)
        }

        async fn update_stream_title(&self, _title: &str) -> Result<(), Error> {
            Ok(())
        }
    }

    // Test the platform capabilities
    let mut platform = MockPlatform::new();

    // Test connection
    platform.connect().await?;
    assert!(matches!(platform.get_connection_status().await?, ConnectionStatus::Connected));

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
