use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SettingsTab {
    Connection,
    General,
    Platforms,
    CustomizeUI,
    Audio,
    StreamOverlay,
    QuickActions,
    Plugins,
    About,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatSide {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct StreamerListEntry {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UISettings {
    pub main_stream_chat_side: ChatSide,
    pub secondary_stream_chat_side: ChatSide,
    pub main_chat_enabled: bool,
    pub secondary_chat_enabled: bool,
    pub multistream_viewer_list: Vec<StreamerListEntry>,
    pub swap_halves: bool,
    pub tab_order: Vec<SettingsTab>,
    pub tab_reorder_unlocked: bool,
    pub tab_enabled: HashMap<SettingsTab, bool>,
}

#[derive(Debug, Clone)]
pub struct AudioSettings {
    pub master_volume: f32,
    pub alert_volume: f32,
    pub tts_volume: f32,
    pub mute_alerts: bool,
    pub mute_tts: bool,
    pub audio_device: String,
}

#[derive(Debug, Clone)]
pub struct StreamOverlaySettings {
    pub show_chat: bool,
    pub chat_opacity: f32,
    pub chat_position_x: f32,
    pub chat_position_y: f32,
    pub chat_width: f32,
    pub chat_height: f32,
    pub show_alerts: bool,
    pub alert_position: String,
    pub alert_duration: f32,
}

impl Default for UISettings {
    fn default() -> Self {
        Self {
            main_stream_chat_side: ChatSide::Left,
            secondary_stream_chat_side: ChatSide::Left,
            main_chat_enabled: true,
            secondary_chat_enabled: true,
            multistream_viewer_list: Vec::new(),
            swap_halves: false,
            tab_order: vec![
                SettingsTab::Connection,
                SettingsTab::General,
                SettingsTab::Platforms,
                SettingsTab::CustomizeUI,
                SettingsTab::Audio,
                SettingsTab::StreamOverlay,
                SettingsTab::QuickActions,
                SettingsTab::Plugins,
                SettingsTab::About,
            ],
            tab_reorder_unlocked: false,
            tab_enabled: {
                let mut map = HashMap::new();
                map.insert(SettingsTab::Connection, true);
                map.insert(SettingsTab::General, true);
                map.insert(SettingsTab::Platforms, true);
                map.insert(SettingsTab::CustomizeUI, true);
                map.insert(SettingsTab::Audio, true);
                map.insert(SettingsTab::StreamOverlay, true);
                map.insert(SettingsTab::QuickActions, true);
                map.insert(SettingsTab::Plugins, true);
                map.insert(SettingsTab::About, true);
                map
            },
        }
    }
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            alert_volume: 0.8,
            tts_volume: 0.7,
            mute_alerts: false,
            mute_tts: false,
            audio_device: "Default".to_string(),
        }
    }
}

impl Default for StreamOverlaySettings {
    fn default() -> Self {
        Self {
            show_chat: true,
            chat_opacity: 0.8,
            chat_position_x: 10.0,
            chat_position_y: 10.0,
            chat_width: 400.0,
            chat_height: 600.0,
            show_alerts: true,
            alert_position: "Top Center".to_string(),
            alert_duration: 5.0,
        }
    }
}