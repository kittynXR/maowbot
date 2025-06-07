use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsInstance {
    pub instance_number: u32,
    pub host: String,
    pub port: u16,
    pub use_ssl: bool,
    pub password: Option<String>,
    pub use_password: bool,
}

impl Default for ObsInstance {
    fn default() -> Self {
        Self {
            instance_number: 1,
            host: "127.0.0.1".to_string(),
            port: 4455,
            use_ssl: false,
            password: None,
            use_password: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsScene {
    pub name: String,
    pub index: usize,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsSource {
    pub name: String,
    pub id: String,
    pub kind: String,
    pub is_visible: bool,
    pub scene_name: Option<String>,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsStreamStatus {
    pub is_streaming: bool,
    pub stream_time_ms: Option<u64>,
    pub bytes_sent: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsRecordStatus {
    pub is_recording: bool,
    pub record_time_ms: Option<u64>,
    pub bytes_written: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ObsConnectionInfo {
    pub instance_number: u32,
    pub connected: bool,
    pub version: Option<String>,
}