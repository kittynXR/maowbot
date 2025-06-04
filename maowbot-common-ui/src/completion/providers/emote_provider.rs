// Completion provider for emotes (Twitch, 7TV, BTTV, FFZ)
use crate::completion::{CompletionProvider, CompletionItem, CompletionCategory, CompletionContext, CompletionScope};
use crate::GrpcClient;
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmoteData {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub url: Option<String>,
}

pub struct EmoteCompletionProvider {
    client: Arc<GrpcClient>,
    // Cache of emotes per channel
    cache: Arc<tokio::sync::RwLock<HashMap<String, Vec<EmoteData>>>>,
}

impl EmoteCompletionProvider {
    pub fn new(client: Arc<GrpcClient>) -> Self {
        Self {
            client,
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
    
    async fn fetch_channel_emotes(&self, channel: &str) -> Vec<EmoteData> {
        let mut emotes = Vec::new();
        
        // Fetch 7TV emotes
        if let Ok(seventv_emotes) = self.fetch_7tv_emotes(channel).await {
            emotes.extend(seventv_emotes);
        }
        
        // Fetch BTTV emotes
        if let Ok(bttv_emotes) = self.fetch_bttv_emotes(channel).await {
            emotes.extend(bttv_emotes);
        }
        
        // Fetch FFZ emotes
        if let Ok(ffz_emotes) = self.fetch_ffz_emotes(channel).await {
            emotes.extend(ffz_emotes);
        }
        
        // TODO: Fetch Twitch emotes from API or cache
        
        emotes
    }
    
    async fn fetch_7tv_emotes(&self, channel: &str) -> Result<Vec<EmoteData>, Box<dyn std::error::Error + Send + Sync>> {
        // This would call the 7TV API
        // For now, return empty vec
        Ok(vec![])
    }
    
    async fn fetch_bttv_emotes(&self, channel: &str) -> Result<Vec<EmoteData>, Box<dyn std::error::Error + Send + Sync>> {
        // This would call the BTTV API
        // For now, return empty vec
        Ok(vec![])
    }
    
    async fn fetch_ffz_emotes(&self, channel: &str) -> Result<Vec<EmoteData>, Box<dyn std::error::Error + Send + Sync>> {
        // This would call the FFZ API
        // For now, return empty vec
        Ok(vec![])
    }
}

#[async_trait]
impl CompletionProvider for EmoteCompletionProvider {
    fn name(&self) -> &str {
        "emotes"
    }
    
    fn is_applicable(&self, context: &CompletionContext) -> bool {
        // For chat contexts, complete emotes when:
        // 1. Not typing a command
        // 2. Typing a word that could be an emote
        // 3. Or typing : for shortcode completion
        matches!(
            &context.scope,
            CompletionScope::TwitchChat { .. } | 
            CompletionScope::DiscordChat { .. } |
            CompletionScope::OverlayChat { .. }
        ) && (!context.is_command() || context.is_emote_shortcode())
    }
    
    async fn provide_completions(
        &self,
        context: &CompletionContext,
        prefix: &str,
    ) -> Result<Vec<CompletionItem>, Box<dyn std::error::Error + Send + Sync>> {
        let channel = context.channel().unwrap_or_default();
        
        // Check cache first
        let cache_key = channel.to_string();
        let cached = {
            let cache = self.cache.read().await;
            cache.get(&cache_key).cloned()
        };
        
        let emotes = if let Some(cached_emotes) = cached {
            cached_emotes
        } else {
            // Fetch and cache
            let fetched = self.fetch_channel_emotes(channel).await;
            {
                let mut cache = self.cache.write().await;
                cache.insert(cache_key, fetched.clone());
            }
            fetched
        };
        
        // Filter by prefix
        let search_prefix = prefix.strip_prefix(':').unwrap_or(prefix).to_lowercase();
        
        let mut items: Vec<CompletionItem> = emotes
            .into_iter()
            .filter(|emote| emote.name.to_lowercase().starts_with(&search_prefix))
            .map(|emote| {
                let category = match emote.provider.as_str() {
                    "7tv" => CompletionCategory::SevenTVEmote,
                    "bttv" => CompletionCategory::BttvEmote,
                    "ffz" => CompletionCategory::FfzEmote,
                    "twitch" => CompletionCategory::TwitchEmote,
                    _ => CompletionCategory::Emote,
                };
                
                CompletionItem {
                    replacement: emote.name.clone(),
                    display: emote.name.clone(),
                    description: Some(format!("{} emote", emote.provider)),
                    category: category.clone(),
                    icon: Some(category.icon().to_string()),
                    priority: 80,
                    metadata: [
                        ("provider".to_string(), emote.provider),
                        ("id".to_string(), emote.id),
                    ].into_iter()
                        .chain(emote.url.map(|url| ("url".to_string(), url)))
                        .collect(),
                }
            })
            .collect();
        
        // Sort by relevance
        items.sort_by(|a, b| {
            // Exact matches first
            let a_exact = a.replacement.to_lowercase() == search_prefix;
            let b_exact = b.replacement.to_lowercase() == search_prefix;
            
            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    // Then by priority
                    b.priority.cmp(&a.priority)
                        .then_with(|| a.replacement.len().cmp(&b.replacement.len()))
                }
            }
        });
        
        Ok(items)
    }
    
    fn cache_duration(&self) -> std::time::Duration {
        // Cache emotes for 30 minutes
        std::time::Duration::from_secs(1800)
    }
}