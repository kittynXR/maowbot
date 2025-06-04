// Unified completion system for all UI components
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use async_trait::async_trait;

pub mod context;
pub mod providers;
pub mod engine;

pub use context::{CompletionContext, CompletionScope};
pub use engine::{CompletionEngine, CompletionEngineBuilder};

/// A completion candidate with metadata
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The text to insert when selected
    pub replacement: String,
    /// The text to display in the completion list
    pub display: String,
    /// Optional description or preview
    pub description: Option<String>,
    /// Category for grouping (e.g., "command", "emote", "user")
    pub category: CompletionCategory,
    /// Optional icon or emoji to display
    pub icon: Option<String>,
    /// Priority for sorting (higher = shown first)
    pub priority: i32,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompletionCategory {
    Command,
    Subcommand,
    Username,
    Emote,
    TwitchEmote,
    SevenTVEmote,
    BttvEmote,
    FfzEmote,
    Argument,
    File,
    Custom(String),
}

impl CompletionCategory {
    pub fn icon(&self) -> &str {
        match self {
            Self::Command => "⚡",
            Self::Subcommand => "▸",
            Self::Username => "@",
            Self::Emote | Self::TwitchEmote => "😀",
            Self::SevenTVEmote => "7️⃣",
            Self::BttvEmote => "🅱️",
            Self::FfzEmote => "🦊",
            Self::Argument => "•",
            Self::File => "📄",
            Self::Custom(_) => "✦",
        }
    }
}

/// Trait for completion providers
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    /// Get the name of this provider
    fn name(&self) -> &str;
    
    /// Check if this provider should be active for the given context
    fn is_applicable(&self, context: &CompletionContext) -> bool;
    
    /// Provide completions for the given context
    async fn provide_completions(
        &self,
        context: &CompletionContext,
        prefix: &str,
    ) -> Result<Vec<CompletionItem>, Box<dyn std::error::Error + Send + Sync>>;
    
    /// Get the cache duration for this provider's results
    fn cache_duration(&self) -> Duration {
        Duration::from_secs(300) // Default 5 minutes
    }
}

/// Cache for completion results
pub struct CompletionCache {
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

struct CacheEntry {
    items: Vec<CompletionItem>,
    timestamp: Instant,
    ttl: Duration,
}

impl CompletionCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub fn get(&self, key: &str) -> Option<Vec<CompletionItem>> {
        let entries = self.entries.read().unwrap();
        if let Some(entry) = entries.get(key) {
            if entry.timestamp.elapsed() < entry.ttl {
                return Some(entry.items.clone());
            }
        }
        None
    }
    
    pub fn set(&self, key: String, items: Vec<CompletionItem>, ttl: Duration) {
        let mut entries = self.entries.write().unwrap();
        entries.insert(key, CacheEntry {
            items,
            timestamp: Instant::now(),
            ttl,
        });
    }
    
    pub fn invalidate(&self, pattern: &str) {
        let mut entries = self.entries.write().unwrap();
        entries.retain(|k, _| !k.contains(pattern));
    }
}

/// Configuration for the completion system
#[derive(Debug, Clone)]
pub struct CompletionConfig {
    /// Maximum number of items to show
    pub max_items: usize,
    /// Enable fuzzy matching
    pub fuzzy_matching: bool,
    /// Minimum prefix length before showing completions
    pub min_prefix_length: usize,
    /// Show descriptions in completion list
    pub show_descriptions: bool,
    /// Show category icons
    pub show_icons: bool,
    /// Group items by category
    pub group_by_category: bool,
    /// Case sensitive matching
    pub case_sensitive: bool,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            max_items: 20,
            fuzzy_matching: true,
            min_prefix_length: 1,
            show_descriptions: true,
            show_icons: true,
            group_by_category: true,
            case_sensitive: false,
        }
    }
}