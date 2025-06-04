// Main completion engine that coordinates providers
use super::{
    CompletionProvider, CompletionItem, CompletionContext, 
    CompletionCache, CompletionConfig, CompletionCategory
};
use std::sync::Arc;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

pub struct CompletionEngine {
    providers: Vec<Box<dyn CompletionProvider>>,
    cache: Arc<CompletionCache>,
    config: CompletionConfig,
    fuzzy_matcher: SkimMatcherV2,
}

impl CompletionEngine {
    pub fn new(config: CompletionConfig) -> Self {
        Self {
            providers: Vec::new(),
            cache: Arc::new(CompletionCache::new()),
            config,
            fuzzy_matcher: SkimMatcherV2::default(),
        }
    }
    
    /// Register a completion provider
    pub fn register_provider(&mut self, provider: Box<dyn CompletionProvider>) {
        tracing::info!("Registered completion provider: {}", provider.name());
        self.providers.push(provider);
    }
    
    /// Get completions for the given context
    pub async fn get_completions(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        let prefix = context.current_word();
        
        // Skip if prefix is too short
        if prefix.len() < self.config.min_prefix_length && !prefix.is_empty() {
            return vec![];
        }
        
        let mut all_items = Vec::new();
        
        // Gather completions from all applicable providers
        for provider in &self.providers {
            if !provider.is_applicable(context) {
                continue;
            }
            
            // Try cache first
            let cache_key = format!("{}:{}:{}", provider.name(), context.scope_key(), prefix);
            
            if let Some(cached) = self.cache.get(&cache_key) {
                all_items.extend(cached);
                continue;
            }
            
            // Fetch from provider
            match provider.provide_completions(context, prefix).await {
                Ok(items) => {
                    self.cache.set(
                        cache_key,
                        items.clone(),
                        provider.cache_duration()
                    );
                    all_items.extend(items);
                }
                Err(e) => {
                    tracing::warn!(
                        "Completion provider '{}' failed: {}", 
                        provider.name(), 
                        e
                    );
                }
            }
        }
        
        // Apply fuzzy matching if enabled
        if self.config.fuzzy_matching && !prefix.is_empty() {
            self.apply_fuzzy_matching(&mut all_items, prefix);
        }
        
        // Sort by priority and relevance
        all_items.sort_by(|a, b| {
            b.priority.cmp(&a.priority)
                .then_with(|| a.replacement.len().cmp(&b.replacement.len()))
        });
        
        // Limit results
        all_items.truncate(self.config.max_items);
        
        // Group by category if enabled
        if self.config.group_by_category {
            self.group_by_category(all_items)
        } else {
            all_items
        }
    }
    
    /// Clear cache for a specific pattern
    pub fn invalidate_cache(&self, pattern: &str) {
        self.cache.invalidate(pattern);
    }
    
    fn apply_fuzzy_matching(&self, items: &mut Vec<CompletionItem>, prefix: &str) {
        let prefix_lower = if self.config.case_sensitive {
            prefix.to_string()
        } else {
            prefix.to_lowercase()
        };
        
        // Score each item
        let mut scored_items: Vec<(CompletionItem, i64)> = items
            .drain(..)
            .filter_map(|item| {
                let target = if self.config.case_sensitive {
                    item.replacement.clone()
                } else {
                    item.replacement.to_lowercase()
                };
                
                self.fuzzy_matcher
                    .fuzzy_match(&target, &prefix_lower)
                    .map(|score| (item, score))
            })
            .collect();
        
        // Sort by score (descending)
        scored_items.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Extract items
        *items = scored_items.into_iter().map(|(item, _)| item).collect();
    }
    
    fn group_by_category(&self, mut items: Vec<CompletionItem>) -> Vec<CompletionItem> {
        // Group items by category while preserving order within categories
        let mut grouped: Vec<(CompletionCategory, Vec<CompletionItem>)> = Vec::new();
        
        for item in items {
            if let Some(group) = grouped.iter_mut().find(|(cat, _)| *cat == item.category) {
                group.1.push(item);
            } else {
                let category = item.category.clone();
                grouped.push((category, vec![item]));
            }
        }
        
        // Flatten back to single list
        grouped.into_iter()
            .flat_map(|(_, items)| items)
            .collect()
    }
}

impl CompletionContext {
    /// Get a cache key for this context's scope
    pub fn scope_key(&self) -> String {
        match &self.scope {
            crate::completion::CompletionScope::TuiCommand => "tui".to_string(),
            crate::completion::CompletionScope::GuiCommand => "gui".to_string(),
            crate::completion::CompletionScope::TwitchChat { channel } => format!("twitch:{}", channel),
            crate::completion::CompletionScope::DiscordChat { guild_id, channel_id } => {
                format!("discord:{}:{}", guild_id, channel_id)
            }
            crate::completion::CompletionScope::VRChatOSC => "vrchat".to_string(),
            crate::completion::CompletionScope::OverlayChat { platform, channel } => {
                format!("overlay:{}:{}", platform, channel)
            }
        }
    }
}

/// Builder for creating a completion engine with providers
pub struct CompletionEngineBuilder {
    config: CompletionConfig,
    providers: Vec<Box<dyn CompletionProvider>>,
}

impl CompletionEngineBuilder {
    pub fn new() -> Self {
        Self {
            config: CompletionConfig::default(),
            providers: Vec::new(),
        }
    }
    
    pub fn with_config(mut self, config: CompletionConfig) -> Self {
        self.config = config;
        self
    }
    
    pub fn with_provider(mut self, provider: Box<dyn CompletionProvider>) -> Self {
        self.providers.push(provider);
        self
    }
    
    pub fn build(self) -> CompletionEngine {
        let mut engine = CompletionEngine::new(self.config);
        for provider in self.providers {
            engine.register_provider(provider);
        }
        engine
    }
}