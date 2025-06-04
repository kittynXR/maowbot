// Completion context to track where completion is happening
use std::collections::HashMap;

/// The scope where completion is being requested
#[derive(Debug, Clone, PartialEq)]
pub enum CompletionScope {
    /// TUI command line
    TuiCommand,
    /// Twitch chat in a specific channel
    TwitchChat { channel: String },
    /// Discord chat in a specific channel
    DiscordChat { guild_id: String, channel_id: String },
    /// VRChat OSC chatbox
    VRChatOSC,
    /// GUI command input
    GuiCommand,
    /// Overlay chat input
    OverlayChat { platform: String, channel: String },
}

/// Context information for completion requests
#[derive(Debug, Clone)]
pub struct CompletionContext {
    /// Where the completion is happening
    pub scope: CompletionScope,
    /// The full input line
    pub input: String,
    /// Cursor position in the input
    pub cursor_position: usize,
    /// User requesting completion (if applicable)
    pub user_id: Option<String>,
    /// User roles/permissions
    pub user_roles: Vec<String>,
    /// Platform-specific metadata
    pub metadata: HashMap<String, String>,
    /// Whether the stream is online (for command availability)
    pub is_stream_online: bool,
}

impl CompletionContext {
    pub fn new(scope: CompletionScope, input: String, cursor_position: usize) -> Self {
        Self {
            scope,
            input,
            cursor_position,
            user_id: None,
            user_roles: vec![],
            metadata: HashMap::new(),
            is_stream_online: false,
        }
    }
    
    /// Get the text before the cursor
    pub fn text_before_cursor(&self) -> &str {
        &self.input[..self.cursor_position.min(self.input.len())]
    }
    
    /// Get the current word being typed
    pub fn current_word(&self) -> &str {
        let before = self.text_before_cursor();
        before.split_whitespace().last().unwrap_or("")
    }
    
    /// Get all words before the current one
    pub fn previous_words(&self) -> Vec<&str> {
        let before = self.text_before_cursor();
        let mut words: Vec<_> = before.split_whitespace().collect();
        if !before.ends_with(' ') && !words.is_empty() {
            words.pop(); // Remove the partial current word
        }
        words
    }
    
    /// Check if we're in a command context (starts with !)
    pub fn is_command(&self) -> bool {
        self.input.trim_start().starts_with('!')
    }
    
    /// Get the command name if in command context
    pub fn command_name(&self) -> Option<&str> {
        if self.is_command() {
            self.input.trim_start()
                .strip_prefix('!')
                .and_then(|s| s.split_whitespace().next())
        } else {
            None
        }
    }
    
    /// Check if we're completing an @mention
    pub fn is_mention(&self) -> bool {
        self.current_word().starts_with('@')
    }
    
    /// Check if we're completing an emote (starts with :)
    pub fn is_emote_shortcode(&self) -> bool {
        self.current_word().starts_with(':') && !self.current_word().ends_with(':')
    }
    
    /// Get the platform for this context
    pub fn platform(&self) -> Option<&str> {
        match &self.scope {
            CompletionScope::TwitchChat { .. } => Some("twitch"),
            CompletionScope::DiscordChat { .. } => Some("discord"),
            CompletionScope::VRChatOSC => Some("vrchat"),
            CompletionScope::OverlayChat { platform, .. } => Some(platform),
            _ => None,
        }
    }
    
    /// Get the channel for this context
    pub fn channel(&self) -> Option<&str> {
        match &self.scope {
            CompletionScope::TwitchChat { channel } => Some(channel),
            CompletionScope::DiscordChat { channel_id, .. } => Some(channel_id),
            CompletionScope::OverlayChat { channel, .. } => Some(channel),
            _ => None,
        }
    }
}