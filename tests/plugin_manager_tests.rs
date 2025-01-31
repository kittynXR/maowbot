use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use maowbot::plugins::manager::{
    PluginManager, PluginConnection, PluginConnectionInfo
};
use maowbot::plugins::proto::plugs::{
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    PluginStreamRequest, PluginStreamResponse,
    Hello, LogMessage, RequestStatus, RequestCaps,
    SwitchScene, SendChat, Shutdown, AuthError,
    PluginCapability, WelcomeResponse, CapabilityResponse
};
use maowbot::eventbus::{EventBus, BotEvent};
use maowbot::Error;

/// A mock connection that just stores all PluginStreamResponses in memory
/// so we can assert on them.
#[derive(Clone)]
struct MockPluginConnection {
    info: Arc<Mutex<PluginConnectionInfo>>,
    outbound_messages: Arc<Mutex<Vec<PluginStreamResponse>>>,
}

impl MockPluginConnection {
    fn new(name: &str) -> Self {
        let info = PluginConnectionInfo {
            name: name.to_string(),
            capabilities: Vec::new(),
        };
        Self {
            info: Arc::new(Mutex::new(info)),
            outbound_messages: Arc::new(Mutex::new(vec![])),
        }
    }

    /// Helper to see what has been sent to the mock so far.
    async fn sent_messages(&self) -> Vec<PluginStreamResponse> {
        self.outbound_messages.lock().await.clone()
    }
}

#[async_trait]
impl PluginConnection for MockPluginConnection {
    async fn info(&self) -> PluginConnectionInfo {
        self.info.lock().await.clone()
    }

    async fn set_capabilities(&self, caps: Vec<PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = caps;
    }

    async fn set_name(&self, new_name: String) {
        let mut guard = self.info.lock().await;
        guard.name = new_name;
    }

    async fn send(&self, response: PluginStreamResponse) -> Result<(), Error> {
        self.outbound_messages.lock().await.push(response);
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        // For the test, do nothing beyond acknowledging the stop
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Helper to create a PluginManager with optional passphrase and an EventBus.
fn setup_manager(passphrase: Option<String>) -> (Arc<PluginManager>, Arc<EventBus>) {
    let event_bus = Arc::new(EventBus::new());
    let mut pm = PluginManager::new(passphrase);
    pm.set_event_bus(event_bus.clone());
    pm.subscribe_to_event_bus(event_bus.clone());
    (Arc::new(pm), event_bus)
}

/// Test: successful Hello => plugin gets "WelcomeResponse"
#[tokio::test]
async fn test_on_inbound_hello_success() {
    let (pm, _bus) = setup_manager(Some("secret".to_string()));
    let plugin = Arc::new(MockPluginConnection::new("unnamed"));

    // Add the plugin so it shows in plugin_list
    pm.add_plugin_connection(plugin.clone()).await;

    // Inbound Hello with correct passphrase
    let payload = ReqPayload::Hello(Hello {
        plugin_name: "TestPlugin".into(),
        passphrase: "secret".into(),
    });

    pm.on_inbound_message(payload, plugin.clone()).await;

    // The plugin should receive exactly one response => WelcomeResponse
    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);
    match messages[0].payload.as_ref().unwrap() {
        RespPayload::Welcome(WelcomeResponse { bot_name }) => {
            assert_eq!(bot_name, "MaowBot");
        }
        other => panic!("Expected WelcomeResponse, got {:?}", other),
    }

    // Also check the plugin name is now "TestPlugin"
    // (works only if the manager sets the name back into plugin)
    let info = plugin.info().await;
    assert_eq!(info.name, "TestPlugin");
}


/// Test: Hello with WRONG passphrase => plugin gets AuthError, then manager calls stop
#[tokio::test]
async fn test_on_inbound_hello_wrong_passphrase() {
    let (pm, _bus) = setup_manager(Some("secret".to_string()));
    let plugin = Arc::new(MockPluginConnection::new("unnamed"));

    let payload = ReqPayload::Hello(Hello {
        plugin_name: "BadPlugin".into(),
        passphrase: "wrong".into(),
    });

    pm.on_inbound_message(payload, plugin.clone()).await;

    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);
    match messages[0].payload.as_ref().unwrap() {
        RespPayload::AuthError(AuthError { reason }) => {
            assert_eq!(reason, "Invalid passphrase");
        }
        other => panic!("Expected AuthError, got {:?}", other),
    }
}

/// Test: plugin logs a message => we just forward to logging
#[tokio::test]
async fn test_on_inbound_log_message() {
    let (pm, _bus) = setup_manager(None);
    let plugin = Arc::new(MockPluginConnection::new("Logger"));

    let payload = ReqPayload::LogMessage(LogMessage {
        text: "Hello from plugin logs!".to_string()
    });

    // Should just get logged, manager doesn't respond
    pm.on_inbound_message(payload, plugin.clone()).await;
    let messages = plugin.sent_messages().await;
    assert!(messages.is_empty(), "No direct response to LogMessage");
}

/// Test: plugin requests status => manager returns StatusResponse
#[tokio::test]
async fn test_on_inbound_request_status() {
    let (pm, _bus) = setup_manager(None);
    let plugin = Arc::new(MockPluginConnection::new("StatusPlugin"));

    pm.add_plugin_connection(plugin.clone()).await;

    pm.on_inbound_message(ReqPayload::RequestStatus(RequestStatus {}), plugin.clone()).await;
    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);

    match messages[0].payload.as_ref().unwrap() {
        RespPayload::StatusResponse(sr) => {
            assert!(sr.server_uptime <= 5, "uptime is small if test is quick");
            // The only connected plugin is ourselves => ["StatusPlugin"]
            assert_eq!(sr.connected_plugins, vec!["StatusPlugin"]);
        }
        other => panic!("Expected StatusResponse, got {:?}", other),
    }
}

/// Test: plugin requests caps => manager grants some + denies ChatModeration
#[tokio::test]
async fn test_on_inbound_request_caps() {
    let (pm, _bus) = setup_manager(None);
    let plugin = Arc::new(MockPluginConnection::new("Cappy"));

    // Plugin wants RECV=0, SEND=1, SCENE=2, and MOD=3
    // The manager automatically denies ChatModeration=3
    let payload = ReqPayload::RequestCaps(RequestCaps {
        requested: vec![
            PluginCapability::ReceiveChatEvents as i32,
            PluginCapability::SendChat as i32,
            PluginCapability::SceneManagement as i32,
            PluginCapability::ChatModeration as i32,
        ],
    });
    pm.on_inbound_message(payload, plugin.clone()).await;

    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);

    match messages[0].payload.as_ref().unwrap() {
        RespPayload::CapabilityResponse(cr) => {
            let granted: Vec<i32> = cr.granted.clone();
            let denied: Vec<i32> = cr.denied.clone();
            assert_eq!(
                granted,
                vec![
                    PluginCapability::ReceiveChatEvents as i32,
                    PluginCapability::SendChat as i32,
                    PluginCapability::SceneManagement as i32
                ]
            );
            assert_eq!(
                denied,
                vec![PluginCapability::ChatModeration as i32]
            );
        }
        other => panic!("Expected CapabilityResponse, got {:?}", other),
    }

    // Also verify the plugin's stored capabilities
    let info = plugin.info().await;
    assert_eq!(info.capabilities.len(), 3);
    assert!(info.capabilities.contains(&PluginCapability::ReceiveChatEvents));
    assert!(info.capabilities.contains(&PluginCapability::SendChat));
    assert!(info.capabilities.contains(&PluginCapability::SceneManagement));
    assert!(!info.capabilities.contains(&PluginCapability::ChatModeration));
}

/// Test: plugin wants bot to shut down => manager calls event_bus.shutdown()
#[tokio::test]
async fn test_on_inbound_shutdown() {
    let (pm, bus) = setup_manager(None);
    let plugin = Arc::new(MockPluginConnection::new("Shutter"));

    let payload = ReqPayload::Shutdown(Shutdown {});
    assert!(!bus.is_shutdown());

    pm.on_inbound_message(payload, plugin.clone()).await;
    assert!(bus.is_shutdown(), "Expected event bus to be shut down");
}

/// Test: plugin tries to switch scene but lacks `SceneManagement` => gets AuthError
#[tokio::test]
async fn test_on_inbound_switch_scene_denied() {
    let (pm, _bus) = setup_manager(None);
    let plugin = Arc::new(MockPluginConnection::new("NoSceneCap"));

    let payload = ReqPayload::SwitchScene(SwitchScene {
        scene_name: "my_scene".to_string(),
    });
    pm.on_inbound_message(payload, plugin.clone()).await;

    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);
    match messages[0].payload.as_ref().unwrap() {
        RespPayload::AuthError(err) => {
            assert_eq!(err.reason, "No SceneManagement capability");
        }
        other => panic!("Expected AuthError, got {:?}", other),
    }
}

/// Test: plugin sends chat but lacks `SendChat` => get AuthError
#[tokio::test]
async fn test_on_inbound_send_chat_denied() {
    let (pm, _bus) = setup_manager(None);
    let plugin = Arc::new(MockPluginConnection::new("NoSendCap"));

    let payload = ReqPayload::SendChat(SendChat {
        channel: "some_channel".to_string(),
        text: "Hello from plugin".to_string(),
    });
    pm.on_inbound_message(payload, plugin.clone()).await;

    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);
    match messages[0].payload.as_ref().unwrap() {
        RespPayload::AuthError(err) => {
            assert_eq!(err.reason, "No SendChat capability");
        }
        other => panic!("Expected AuthError, got {:?}", other),
    }
}

/// Test: plugin **has** SendChat capability => manager posts a ChatMessage event to the bus.
#[tokio::test]
async fn test_on_inbound_send_chat_granted() {
    let (pm, bus) = setup_manager(None);
    let plugin = Arc::new(MockPluginConnection::new("Sender"));

    // manually set the plugin's capabilities
    plugin.set_capabilities(vec![PluginCapability::SendChat]).await;

    // Subscribe to bus so we can see if ChatMessage gets published
    let mut rx = bus.subscribe(None);

    let payload = ReqPayload::SendChat(SendChat {
        channel: "chanA".to_string(),
        text: "Hello from plugin".to_string(),
    });
    pm.on_inbound_message(payload, plugin.clone()).await;

    // Should not get an AuthError => no response
    let messages = plugin.sent_messages().await;
    assert!(messages.is_empty(), "Should not receive any error or response to SendChat on success");

    // Verify bus got a ChatMessage event
    let evt = rx.recv().await.expect("Should get an event");
    match evt {
        BotEvent::ChatMessage { platform, channel, user, text, .. } => {
            assert_eq!(platform, "plugin");  // manager sets "plugin" as platform
            assert_eq!(channel, "chanA");
            assert_eq!(user, "Sender");
            assert_eq!(text, "Hello from plugin");
        },
        other => panic!("Expected ChatMessage, got {:?}", other),
    }
}
