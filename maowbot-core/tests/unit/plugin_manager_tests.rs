// maowbot-core/tests/unit/plugin_manager_tests.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use maowbot_core::plugins::manager::{PluginManager};
use maowbot_proto::plugs::{
    plugin_stream_request::Payload as ReqPayload,
    plugin_stream_response::Payload as RespPayload,
    Hello, LogMessage, RequestStatus, RequestCaps,
    SwitchScene, SendChat, Shutdown, AuthError,
    PluginCapability, WelcomeResponse, CapabilityResponse
};
use maowbot_core::eventbus::{EventBus, BotEvent};
use maowbot_core::Error;
use maowbot_core::plugins::plugin_connection::{PluginConnection, PluginConnectionInfo};

#[derive(Clone)]
struct MockPluginConnection {
    info: Arc<Mutex<PluginConnectionInfo>>,
    outbound_messages: Arc<Mutex<Vec<String>>>,
}

impl MockPluginConnection {
    fn new(name: &str) -> Self {
        Self {
            info: Arc::new(Mutex::new(PluginConnectionInfo {
                name: name.to_string(),
                capabilities: Vec::new(),
                is_enabled: false,
            })),
            outbound_messages: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn sent_messages(&self) -> Vec<String> {
        self.outbound_messages.lock().await.clone()
    }
}

#[async_trait]
impl PluginConnection for MockPluginConnection {
    async fn info(&self) -> PluginConnectionInfo {
        self.info.lock().await.clone()
    }

    async fn set_capabilities(&self, caps: Vec<maowbot_proto::plugs::PluginCapability>) {
        let mut guard = self.info.lock().await;
        guard.capabilities = caps;
    }

    async fn set_name(&self, new_name: String) {
        let mut guard = self.info.lock().await;
        guard.name = new_name;
    }

    async fn set_enabled(&self, enabled: bool) {
        let mut info = self.info.lock().await;
        info.is_enabled = enabled;
    }

    async fn send(&self, response: maowbot_proto::plugs::PluginStreamResponse) -> Result<(), Error> {
        // Format the payload using Debug so we can later assert its contents.
        let debug_str = format!("{:?}", response.payload);
        self.outbound_messages.lock().await.push(debug_str);
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

async fn setup_manager(passphrase: Option<String>) -> (PluginManager, Arc<EventBus>) {
    let event_bus = Arc::new(EventBus::new());
    let mut pm = PluginManager::new(passphrase);
    pm.set_event_bus(event_bus.clone());
    pm.subscribe_to_event_bus(event_bus.clone()).await;
    (pm, event_bus)
}

#[tokio::test]
async fn test_on_inbound_hello_success() {
    let (mut pm, _bus) = setup_manager(Some("secret".to_string())).await;
    let plugin = Arc::new(MockPluginConnection::new("unnamed"));

    // Add and enable the plugin so that it will process the Hello message.
    pm.add_plugin_connection(plugin.clone()).await;
    plugin.set_enabled(true).await;

    let payload = ReqPayload::Hello(Hello {
        plugin_name: "TestPlugin".into(),
        passphrase: "secret".into(),
    });

    pm.on_inbound_message(payload, plugin.clone()).await;

    let msgs = plugin.sent_messages().await;
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].contains("WelcomeResponse"));
}

#[tokio::test]
async fn test_on_inbound_hello_wrong_passphrase() {
    let (mut pm, _bus) = setup_manager(Some("secret".to_string())).await;
    let plugin = Arc::new(MockPluginConnection::new("unnamed"));

    // Add the plugin to the manager.
    pm.add_plugin_connection(plugin.clone()).await;

    let payload = ReqPayload::Hello(Hello {
        plugin_name: "BadPlugin".into(),
        passphrase: "wrong".into(),
    });

    pm.on_inbound_message(payload, plugin.clone()).await;

    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("AuthError"));
}

#[tokio::test]
async fn test_on_inbound_log_message() {
    let (mut pm, _bus) = setup_manager(None).await;
    let plugin = Arc::new(MockPluginConnection::new("Logger"));

    // Add and enable the plugin so that messages are processed.
    pm.add_plugin_connection(plugin.clone()).await;
    plugin.set_enabled(true).await;

    let payload = ReqPayload::LogMessage(LogMessage {
        text: "Hello from plugin logs!".to_string()
    });

    pm.on_inbound_message(payload, plugin.clone()).await;
    // Log messages do not produce outbound responses.
    assert_eq!(plugin.sent_messages().await.len(), 0);
}

#[tokio::test]
async fn test_on_inbound_request_status() {
    let (mut pm, _bus) = setup_manager(None).await;
    let plugin = Arc::new(MockPluginConnection::new("StatusPlugin"));

    pm.add_plugin_connection(plugin.clone()).await;
    plugin.set_enabled(true).await;

    pm.on_inbound_message(ReqPayload::RequestStatus(RequestStatus {}), plugin.clone()).await;
    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("StatusResponse"));
}

#[tokio::test]
async fn test_on_inbound_request_caps() {
    let (mut pm, _bus) = setup_manager(None).await;
    let plugin = Arc::new(MockPluginConnection::new("Cappy"));

    pm.add_plugin_connection(plugin.clone()).await;
    plugin.set_enabled(true).await;

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
    // Check that the response contains a CapabilityResponse and that the denied vector
    // includes the ChatModeration capability (printed by name).
    assert!(messages[0].contains("CapabilityResponse"));
    assert!(messages[0].contains("denied: [ChatModeration]"));
}

#[tokio::test]
async fn test_on_inbound_shutdown() {
    let (mut pm, bus) = setup_manager(None).await;
    let plugin = Arc::new(MockPluginConnection::new("Shutter"));

    // Add and enable the plugin so that the shutdown message is processed.
    pm.add_plugin_connection(plugin.clone()).await;
    plugin.set_enabled(true).await;

    pm.on_inbound_message(ReqPayload::Shutdown(Shutdown {}), plugin.clone()).await;
    assert!(bus.is_shutdown());
}

#[tokio::test]
async fn test_on_inbound_switch_scene_denied() {
    let (mut pm, _bus) = setup_manager(None).await;
    let plugin = Arc::new(MockPluginConnection::new("NoSceneCap"));

    pm.add_plugin_connection(plugin.clone()).await;
    plugin.set_enabled(true).await;

    let payload = ReqPayload::SwitchScene(SwitchScene {
        scene_name: "my_scene".to_string(),
    });
    pm.on_inbound_message(payload, plugin.clone()).await;

    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("AuthError"));
}

#[tokio::test]
async fn test_on_inbound_send_chat_denied() {
    let (mut pm, _bus) = setup_manager(None).await;
    let plugin = Arc::new(MockPluginConnection::new("NoSendCap"));

    pm.add_plugin_connection(plugin.clone()).await;
    plugin.set_enabled(true).await;

    let payload = ReqPayload::SendChat(SendChat {
        channel: "some_channel".to_string(),
        text: "Hello from plugin".to_string(),
    });
    pm.on_inbound_message(payload, plugin.clone()).await;

    let messages = plugin.sent_messages().await;
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("AuthError"));
}

#[tokio::test]
async fn test_on_inbound_send_chat_granted() {
    let (mut pm, bus) = setup_manager(None).await;
    let plugin = Arc::new(MockPluginConnection::new("Sender"));
    pm.add_plugin_connection(plugin.clone()).await;
    plugin.set_enabled(true).await;
    // Grant the SendChat capability.
    plugin.set_capabilities(vec![PluginCapability::SendChat]).await;

    let mut rx = bus.subscribe(None).await;

    let payload = ReqPayload::SendChat(SendChat {
        channel: "chanA".to_string(),
        text: "Hello from plugin".to_string(),
    });
    pm.on_inbound_message(payload, plugin.clone()).await;

    let messages = plugin.sent_messages().await;
    // When successful, no direct error or response is sent to the plugin.
    assert!(messages.is_empty(), "No error or direct response on success");

    // The event bus should have received a ChatMessage event.
    let evt = rx.recv().await.expect("Should get an event");
    match evt {
        BotEvent::ChatMessage { platform, channel, user, text, .. } => {
            assert_eq!(platform, "plugin");
            assert_eq!(channel, "chanA");
            assert_eq!(user, "Sender");
            assert_eq!(text, "Hello from plugin");
        },
        other => panic!("Expected ChatMessage event, got {:?}", other),
    }
}