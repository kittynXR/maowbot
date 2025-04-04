use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::RwLock;

use crate::models::MemoryEntry;
use crate::traits::{ChatMessage, MemorySystem};

/// In-memory implementation of the MemorySystem trait
pub struct InMemorySystem {
    memories: Arc<RwLock<HashMap<String, Vec<MemoryEntry>>>>,
    max_entries_per_user: usize,
}

impl InMemorySystem {
    /// Create a new in-memory system with specified capacity per user
    pub fn new(max_entries_per_user: usize) -> Self {
        Self {
            memories: Arc::new(RwLock::new(HashMap::new())),
            max_entries_per_user,
        }
    }
}

#[async_trait]
impl MemorySystem for InMemorySystem {
    async fn store(&self, user_id: &str, message: ChatMessage) -> anyhow::Result<()> {
        let mut memories = self.memories.write().await;
        
        let user_memories = memories
            .entry(user_id.to_string())
            .or_insert_with(Vec::new);
        
        let entry = MemoryEntry {
            user_id: user_id.to_string(),
            platform: "unknown".to_string(), // Update this if platform info is available
            timestamp: Utc::now(),
            message,
            metadata: HashMap::new(),
        };
        
        user_memories.push(entry);
        
        // Trim if exceeding max capacity
        if user_memories.len() > self.max_entries_per_user {
            *user_memories = user_memories
                .iter()
                .skip(user_memories.len() - self.max_entries_per_user)
                .cloned()
                .collect();
        }
        
        Ok(())
    }
    
    async fn retrieve(&self, user_id: &str, limit: usize) -> anyhow::Result<Vec<ChatMessage>> {
        let memories = self.memories.read().await;
        
        let user_memories = memories.get(user_id).cloned().unwrap_or_default();
        
        // Get the latest entries up to limit
        let memory_count = user_memories.len();
        let start_idx = if memory_count > limit { memory_count - limit } else { 0 };
        
        let messages = user_memories
            .iter()
            .skip(start_idx)
            .map(|entry| entry.message.clone())
            .collect();
        
        Ok(messages)
    }
    
    async fn clear(&self, user_id: &str) -> anyhow::Result<()> {
        let mut memories = self.memories.write().await;
        memories.remove(user_id);
        Ok(())
    }
    
    async fn summarize(&self, user_id: &str) -> anyhow::Result<String> {
        // Simple implementation for now - could be enhanced with actual summarization
        let memories = self.memories.read().await;
        
        let user_memories = memories.get(user_id).cloned().unwrap_or_default();
        
        if user_memories.is_empty() {
            return Ok("No conversation history".to_string());
        }
        
        let message_count = user_memories.len();
        let earliest = user_memories.first().map(|e| e.timestamp).unwrap_or_default();
        let latest = user_memories.last().map(|e| e.timestamp).unwrap_or_default();
        
        Ok(format!(
            "Conversation history: {} messages from {} to {}",
            message_count,
            earliest.to_rfc3339(),
            latest.to_rfc3339()
        ))
    }
}

/// Manager for different memory systems
pub struct MemoryManager {
    systems: Arc<RwLock<HashMap<String, Arc<dyn MemorySystem>>>>,
    default_system: String,
}

impl MemoryManager {
    /// Create a new memory manager with an in-memory system as default
    pub fn new() -> Self {
        let mut systems = HashMap::new();
        let default_system = "in_memory".to_string();
        
        // Create default in-memory system
        let in_memory = Arc::new(InMemorySystem::new(100));
        systems.insert(default_system.clone(), in_memory as Arc<dyn MemorySystem>);
        
        Self {
            systems: Arc::new(RwLock::new(systems)),
            default_system,
        }
    }
    
    /// Register a new memory system
    pub async fn register_system(
        &self,
        name: impl Into<String>,
        system: Arc<dyn MemorySystem>,
    ) {
        let mut systems = self.systems.write().await;
        systems.insert(name.into(), system);
    }
    
    /// Set the default memory system
    pub fn set_default_system(&mut self, name: impl Into<String>) {
        self.default_system = name.into();
    }
    
    /// Get a memory system by name, or the default if not found
    pub async fn get_system(&self, name: Option<&str>) -> Arc<dyn MemorySystem> {
        let systems = self.systems.read().await;
        
        match name {
            Some(system_name) => systems
                .get(system_name)
                .cloned()
                .unwrap_or_else(|| systems.get(&self.default_system).cloned().unwrap()),
            None => systems.get(&self.default_system).cloned().unwrap(),
        }
    }
    
    /// Store a user message using the default memory system
    pub async fn store_message(&self, user_id: &str, message: ChatMessage) -> anyhow::Result<()> {
        let system = self.get_system(None).await;
        system.store(user_id, message).await
    }
    
    /// Retrieve messages for a user using the default memory system
    pub async fn retrieve_messages(&self, user_id: &str, limit: usize) -> anyhow::Result<Vec<ChatMessage>> {
        let system = self.get_system(None).await;
        system.retrieve(user_id, limit).await
    }
}