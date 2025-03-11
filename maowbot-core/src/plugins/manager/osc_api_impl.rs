use maowbot_osc::MaowOscManager;
use maowbot_osc::oscquery::OscQueryServer; // for is_running()
use maowbot_osc::OscError;

use crate::Error;
use crate::plugins::bot_api::osc_api::{OscApi, OscStatus};
use crate::plugins::manager::core::PluginManager;
use async_trait::async_trait;

#[async_trait]
impl OscApi for PluginManager {
    async fn osc_start(&self) -> Result<(), Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        mgr.start_all()
            .await
            .map_err(|e| Error::Platform(format!("OSC start error: {e:?}")))?;
        Ok(())
    }

    async fn osc_stop(&self) -> Result<(), Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        mgr.stop_all()
            .await
            .map_err(|e| Error::Platform(format!("OSC stop error: {e:?}")))?;
        Ok(())
    }

    async fn osc_status(&self) -> Result<OscStatus, Error> {
        if let Some(mgr) = &self.osc_manager {
            // <--- Retrieve the overall status from our new helper:
            let st = mgr.get_status()
                .await
                .map_err(|e| Error::Platform(format!("OSC status error: {e:?}")))?;

            // For the OSCQuery side:
            let oscq = mgr.oscquery_server.lock().await;
            let is_oscquery_running = oscq.is_running();  // using the accessor
            let port = oscq.http_port;

            // Return a user-friendly OscStatus
            Ok(OscStatus {
                is_running: st.is_running,
                listening_port: st.listening_port,
                is_oscquery_running,
                oscquery_port: Some(port),
                discovered_peers: Vec::new(),
            })
        } else {
            // No manager => default "off" status
            Ok(OscStatus {
                is_running: false,
                listening_port: None,
                is_oscquery_running: false,
                oscquery_port: None,
                discovered_peers: Vec::new(),
            })
        }
    }

    async fn osc_chatbox(&self, message: &str) -> Result<(), Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        let msg = maowbot_osc::vrchat::chatbox::ChatboxMessage {
            text: message.to_string(),
            send_immediately: true,
            play_notification_sound: true,
        };
        maowbot_osc::vrchat::chatbox::send_chatbox_message(mgr, &msg)
            .map_err(|e| Error::Platform(format!("OSC chat error: {e:?}")))?;
        Ok(())
    }

    async fn osc_discover_peers(&self) -> Result<Vec<String>, Error> {
        let mgr = self.osc_manager
            .as_ref()
            .ok_or_else(|| Error::Platform("No OSC manager attached".to_string()))?;
        mgr.discover_local_peers()
            .await
            .map_err(|e| Error::Platform(format!("OSC discover error: {e:?}")))
    }
}
