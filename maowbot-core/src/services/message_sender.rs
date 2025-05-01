use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use tracing::{debug, warn, info};
use uuid::Uuid;
use maowbot_common::models::platform::Platform;
use maowbot_common::models::platform::Platform::TwitchIRC;
use maowbot_common::models::platform::PlatformCredential;
use maowbot_common::traits::repository_traits::CredentialsRepository;
use crate::platforms::manager::PlatformManager;
use crate::Error;
use serde_json::Value;
use lazy_static::lazy_static;
use parking_lot::Mutex;
use once_cell::sync::Lazy;
/* -----------------------------------------------------------
 *  GLOBAL CONTINUATION CACHE
 * -----------------------------------------------------------
 *  All MessageSender instances share this map, allowing the
 *  `!continue` command (handled elsewhere) to pop the next
 *  chunk no matter which MessageSender originally created it.
 * -----------------------------------------------------------
 */
static PENDING_CONTINUATIONS: Lazy<Mutex<HashMap<String, VecDeque<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Maximum length for Twitch chat messages
pub const TWITCH_MAX_MSG_LENGTH: usize = 450;
const MAX_TWITCH_MSG_LEN: usize = 450;

/// Structure to store message context for commands like !sources and !continue
#[derive(Debug, Clone)]
pub struct MessageContext {
    /// The full original message
    pub full_message: String,
    /// The channel this message was sent to
    pub channel: String,
    /// Message segments if it was split due to length
    pub segments: Vec<String>,
    /// Current segment index (for !continue command)
    pub current_segment: usize,
    /// Source citations from AI response
    pub sources: Vec<SourceCitation>,
    /// Channel credentials used to send this message
    pub credential_id: Option<Uuid>,
    /// Timestamp when message was created
    pub timestamp: std::time::SystemTime,
}

/// Source citation for AI responses
#[derive(Debug, Clone)]
pub struct SourceCitation {
    /// The URL of the citation
    pub url: String,
    /// The title of the source
    pub title: String,
}

/// Global storage for message contexts
lazy_static! {
    static ref LAST_MESSAGES: Mutex<HashMap<String, MessageContext>> = Mutex::new(HashMap::new());
}

/// Generic response type for message sending operations
#[derive(Debug, Clone)]
pub struct MessageResponse {
    pub texts: Vec<String>,
    pub respond_credential_id: Option<Uuid>,
    pub platform: String,
    pub channel: String,
}

/// Service for sending messages across different platforms with proper credential selection
pub struct MessageSender {
    pub credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    pub platform_manager: Arc<PlatformManager>,
}

impl MessageSender {
    /// Create a new MessageSender service
    pub fn new(
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
        platform_manager: Arc<PlatformManager>,
    ) -> Self {
        Self {
            credentials_repo,
            platform_manager,
        }
    }

    /// Pop the next queued segment for the given channel (if any).
    /// Returns `Some(next_chunk)` or `None` if nothing is waiting.
    pub fn pop_next_segment(channel: &str) -> Option<String> {
        // parking_lot::Mutex::lock() never returns a Result, so no `.unwrap()`
        let mut map = PENDING_CONTINUATIONS.lock();
        match map.get_mut(channel) {
            Some(queue) => {
                let next = queue.pop_front();
                if queue.is_empty() {
                    map.remove(channel);
                }
                next
            }
            None => None,
        }
    }

    /// Check whether any continuation text is pending for the channel.
       pub fn has_pending(channel: &str) -> bool {
        let map = PENDING_CONTINUATIONS.lock();
        map.get(channel).map_or(false, |q| !q.is_empty())
    }

    /// Determine which credential to use for sending messages on a given platform
    /// 
    /// Follows these rules:
    /// 1. If specified_credential_id is provided and valid, use that
    /// 2. Find the first bot credential for the platform
    /// 3. Find the first broadcaster credential for the platform
    /// 4. Use the specified message_sender_user_id's credential if available
    /// 5. Return None if no suitable credential is found
    pub async fn select_response_credential(
        &self,
        platform: &Platform,
        specified_credential_id: Option<Uuid>,
        message_sender_user_id: Uuid,
    ) -> Result<Option<PlatformCredential>, Error> {
        // #1: if a specific credential ID is specified, try to use it first
        if let Some(cid) = specified_credential_id {
            if let Ok(Some(c)) = self.credentials_repo.get_credential_by_id(cid).await {
                if c.platform == *platform {
                    debug!("Using specified credential: {} ({})", c.user_name, c.credential_id);
                    return Ok(Some(c));
                }
            }
        }

        // #2: Get all credentials for this platform
        let all_creds = self.credentials_repo.list_credentials_for_platform(platform).await?;
        
        // If no credentials exist for this platform, return None
        if all_creds.is_empty() {
            warn!("No credentials found for platform {:?}", platform);
            return Ok(None);
        }

        // #3: Find the first bot credential
        if let Some(bot_cred) = all_creds.iter().find(|c| c.is_bot) {
            debug!("Using bot credential: {} ({})", bot_cred.user_name, bot_cred.credential_id);
            return Ok(Some(bot_cred.clone()));
        }

        // #4: Find the first broadcaster credential
        if let Some(broadcaster_cred) = all_creds.iter().find(|c| c.is_broadcaster) {
            debug!("Using broadcaster credential: {} ({})", broadcaster_cred.user_name, broadcaster_cred.credential_id);
            return Ok(Some(broadcaster_cred.clone()));
        }

        // #5: Try to use the message sender's own credential
        let maybe_same_user_cred = self.credentials_repo.get_credentials(
            platform,
            message_sender_user_id
        ).await?;
        
        if let Some(c) = maybe_same_user_cred {
            debug!("Using message sender's credential: {} ({})", c.user_name, c.credential_id);
            return Ok(Some(c));
        }

        // #6: If nothing else works, use the first credential we found
        if !all_creds.is_empty() {
            debug!("Using first available credential: {} ({})", all_creds[0].user_name, all_creds[0].credential_id);
            return Ok(Some(all_creds[0].clone()));
        }

        // If we can't find any suitable credential, just return None
        warn!("No suitable credential found for platform {:?}", platform);
        Ok(None)
    }

    /// Split a message into segments that fit within Twitch's message length limits
    pub fn split_message(&self, message: &str) -> Vec<String> {
        if message.len() <= TWITCH_MAX_MSG_LENGTH {
            return vec![message.to_string()];
        }
        
        let mut segments = Vec::new();
        let mut current_pos = 0;
        
        while current_pos < message.len() {
            // Calculate the maximum possible segment length
            let max_segment_length = std::cmp::min(TWITCH_MAX_MSG_LENGTH - 3, message.len() - current_pos); // -3 for ellipsis
            
            // Start with the maximum allowed segment
            let mut end_pos = current_pos + max_segment_length;
            
            // If we're not at the end of the message, we need to find a good break point
            if end_pos < message.len() {
                // Try to find a good break point in the last 150 characters of the current segment
                // This gives us more room to find natural breaking points
                let search_start = if max_segment_length > 150 { end_pos - 150 } else { current_pos };
                
                // Look for these break strings in order of preference
                let mut found_break = false;
                for break_str in &[". ", "! ", "? ", "; ", ", ", " "] {
                    // Find the last occurrence of this break string in our search space
                    if let Some(pos) = message[search_start..end_pos].rfind(break_str) {
                        // Adjust the end position to include the break string
                        end_pos = search_start + pos + break_str.len();
                        found_break = true;
                        break;
                    }
                }
                
                // If we couldn't find any break point, at least avoid cutting words
                if !found_break && end_pos < message.len() {
                    // Find the last space before our limit
                    if let Some(pos) = message[current_pos..end_pos].rfind(' ') {
                        end_pos = current_pos + pos + 1; // Include the space
                    }
                    // If there's no space, we'll just cut at the maximum allowed length
                }
            }
            
            // Extract the segment and add ellipsis if needed
            let mut segment = message[current_pos..end_pos].to_string();
            if end_pos < message.len() {
                segment.push_str("...");
            }
            
            segments.push(segment);
            current_pos = end_pos;
        }
        
        // If we have more than one segment, add "(1/3)" etc. to the beginning of each segment
        if segments.len() > 1 {
            // Get the total number of segments first to avoid borrowing issues
            let total_segments = segments.len();
            
            for (i, segment) in segments.iter_mut().enumerate() {
                *segment = format!("({}/{}) {}", i+1, total_segments, segment);
            }
        }
        
        segments
    }
    
    /// Extract source citations from OpenAI API response
    pub fn extract_sources(&self, response: &Value) -> Vec<SourceCitation> {
        let mut sources = Vec::new();
        
        // Check if the response has annotations
        if let Some(annotations) = response.get("annotations").and_then(|a| a.as_array()) {
            for annotation in annotations {
                // Try the standard OpenAI format first (url_citation)
                if let Some(url_citation) = annotation.get("url_citation") {
                    let url = url_citation.get("url").and_then(|u| u.as_str()).unwrap_or("unknown").to_string();
                    let title = url_citation.get("title").and_then(|t| t.as_str()).unwrap_or("Unknown source").to_string();
                    
                    sources.push(SourceCitation { url, title });
                }
                // Also check for citations in other formats
                else if let Some(citation) = annotation.get("citation") {
                    if let Some(url) = citation.get("url").and_then(|u| u.as_str()) {
                        let title = citation.get("title")
                            .and_then(|t| t.as_str())
                            .or_else(|| citation.get("name").and_then(|n| n.as_str()))
                            .unwrap_or("Unknown source")
                            .to_string();
                        
                        sources.push(SourceCitation { url: url.to_string(), title });
                    }
                }
                // Check for direct url field
                else if let Some(url) = annotation.get("url").and_then(|u| u.as_str()) {
                    let title = annotation.get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("Unknown source")
                        .to_string();
                    
                    sources.push(SourceCitation { url: url.to_string(), title });
                }
            }
        }
        
        // Also check for different response formats (some API responses use different structure)
        if sources.is_empty() {
            // Check for sources field at the top level
            if let Some(sources_array) = response.get("sources").and_then(|s| s.as_array()) {
                for source in sources_array {
                    let url = source.get("url").and_then(|u| u.as_str()).unwrap_or("unknown").to_string();
                    let title = source.get("title")
                        .and_then(|t| t.as_str())
                        .or_else(|| source.get("name").and_then(|n| n.as_str()))
                        .unwrap_or("Unknown source")
                        .to_string();
                    
                    sources.push(SourceCitation { url, title });
                }
            }
            
            // Check for citations field at the top level
            if let Some(citations_array) = response.get("citations").and_then(|c| c.as_array()) {
                for citation in citations_array {
                    let url = citation.get("url").and_then(|u| u.as_str()).unwrap_or("unknown").to_string();
                    let title = citation.get("title")
                        .and_then(|t| t.as_str())
                        .or_else(|| citation.get("name").and_then(|n| n.as_str()))
                        .unwrap_or("Unknown source")
                        .to_string();
                    
                    sources.push(SourceCitation { url, title });
                }
            }
        }
        
        // Log the sources we found
        debug!("Extracted {} sources from AI response", sources.len());
        for (i, source) in sources.iter().enumerate() {
            debug!("Source {}: {} - {}", i+1, source.title, source.url);
        }
        
        sources
    }
    
    /// Handles the !continue command - sends the next segment of a truncated message
    pub async fn handle_continue_command(
        &self,
        channel: &str,
        credential_id: Option<Uuid>,
        message_sender_user_id: Uuid,
    ) -> Result<bool, Error> {
        // Normalize both forms so we can look-up either
        let channel_with_hash = if channel.starts_with('#') {
            channel.to_string()
        } else {
            format!("#{}", channel)
        };

        /* ----------------------------------------------------------------
         *  STEP 1 – try the PENDING_CONTINUATIONS queue (used by the
         *  standard send_twitch_message splitter).
         * ---------------------------------------------------------------- */
        if let Some(next_seg) = MessageSender::pop_next_segment(&channel_with_hash) {
            // Send this next chunk
            self.send_twitch_message(
                channel,             // user-supplied form (keeps consistency)
                &next_seg,
                credential_id,
                message_sender_user_id,
            )
            .await?;

            // If more remain, remind the user
            if MessageSender::has_pending(&channel_with_hash) {
                self.send_twitch_message(
                    channel,
                    "Type !continue for more",
                    credential_id,
                    message_sender_user_id,
                )
                .await
                .ok();
            }
            return Ok(true);
        }

        /* ----------------------------------------------------------------
         *  STEP 2 – fall back to the older LAST_MESSAGES context
         *  (used by send_ai_response_to_twitch).
         * ---------------------------------------------------------------- */
        let (segment_opt, ctx_cred_id, has_more) = {
            let mut map = LAST_MESSAGES.lock();
            if let Some(ctx) = map.get_mut(&channel_with_hash) {
                if ctx.current_segment + 1 < ctx.segments.len() {
                    ctx.current_segment += 1;
                    (
                        Some(ctx.segments[ctx.current_segment].clone()),
                        ctx.credential_id,
                        ctx.current_segment + 1 < ctx.segments.len(),
                    )
                } else {
                    (None, ctx.credential_id, false)
                }
            } else {
                (None, None, false)
            }
        };

        if let Some(seg) = segment_opt {
            self.send_twitch_message(
                channel,
                &seg,
                credential_id.or(ctx_cred_id),
                message_sender_user_id,
            )
            .await?;

            if has_more {
                self.send_twitch_message(
                    channel,
                    "Type !continue for more",
                    credential_id.or(ctx_cred_id),
                    message_sender_user_id,
                )
                .await
                .ok();
            }
            return Ok(true);
        }

        // Nothing available
        Ok(false)
    }
    
    /// Handles the !sources command - sends the sources for an AI response
    pub async fn handle_sources_command(&self, channel: &str, credential_id: Option<Uuid>, message_sender_user_id: Uuid) -> Result<bool, Error> {
        let normalized_channel = if channel.starts_with('#') {
            channel.to_string()
        } else {
            format!("#{}", channel)
        };
        
        // Debug log for troubleshooting
        debug!("Looking for sources context for channel: {} (normalized: {})", channel, normalized_channel);
        
        // Get the sources and credential_id from the context atomically
        let (sources, context_credential_id, has_context) = {
            let contexts = LAST_MESSAGES.lock();
            
            // Debug log the available contexts for troubleshooting
            debug!("Available message contexts: {:?}", contexts.keys().collect::<Vec<_>>());
            
            if let Some(context) = contexts.get(&normalized_channel) {
                // Clone the sources and credential_id from the context to avoid holding the lock across await points
                debug!("Found sources context with {} sources", context.sources.len());
                (context.sources.clone(), context.credential_id, true)
            } else {
                // Try with alternative channel name format (without #)
                let alt_channel = channel.trim_start_matches('#').to_string();
                debug!("Trying alternative channel format: {}", alt_channel);
                
                if let Some(context) = contexts.get(&alt_channel) {
                    debug!("Found sources context in alt format with {} sources", context.sources.len());
                    (context.sources.clone(), context.credential_id, true)
                } else {
                    debug!("No sources context found for channel");
                    (Vec::new(), None, false)
                }
            }
        };
        
        // If we have no context at all, return a specific message
        if !has_context {
            // No message context exists at all
            self.send_twitch_message(
                channel,
                "No recent AI message found. Sources are only available for recent AI responses.",
                credential_id,
                message_sender_user_id
            ).await?;
            return Ok(true);
        }
        
        // If we have context but no sources
        if sources.is_empty() {
            self.send_twitch_message(
                channel,
                "No sources were used in the most recent AI response.",
                credential_id.or(context_credential_id),
                message_sender_user_id
            ).await?;
            return Ok(true);
        }
        
        // We have sources, let's display them
        // Construct sources message
        let mut sources_msg = "Sources: ".to_string();
        for (i, source) in sources.iter().enumerate() {
            // Format the source with title and URL
            let source_text = format!("{}. {} [{}]", i+1, source.title, source.url);
            
            // Check if adding this source would exceed the limit
            if sources_msg.len() + source_text.len() > TWITCH_MAX_MSG_LENGTH {
                // Send what we have so far
                self.send_twitch_message(
                    channel,
                    &sources_msg,
                    credential_id.or(context_credential_id),
                    message_sender_user_id
                ).await?;
                
                // Start a new message
                sources_msg = format!("More sources: {}. {} [{}]", i+1, source.title, source.url);
            } else {
                if i > 0 {
                    sources_msg.push_str(" | ");
                }
                sources_msg.push_str(&format!("{}. {} [{}]", i+1, source.title, source.url));
            }
        }
        
        // Send any remaining sources
        if !sources_msg.is_empty() {
            self.send_twitch_message(
                channel,
                &sources_msg,
                credential_id.or(context_credential_id),
                message_sender_user_id
            ).await?;
        }
        
        Ok(true)
    }

    /// Send a message to Twitch IRC, handling truncation if needed
    pub async fn send_twitch_message(
        &self,
        channel: &str,
        message: &str,
        specified_credential_id: Option<Uuid>,
        message_sender_user_id: Uuid,
    ) -> Result<(), Error> {
        info!("Attempting to send Twitch message to channel: {}", channel);

        // Make sure the channel name starts with a # prefix for Twitch IRC
        let channel_with_hash = if !channel.starts_with('#') {
            format!("#{}", channel)
        } else {
            channel.to_string()
        };

        // ----------------------------------------------------------------
        // 1) Split message into <=450-char chunks on word-boundaries
        // ----------------------------------------------------------------
        let segments = Self::split_into_chunks(message, MAX_TWITCH_MSG_LEN);

        // Nothing to send – rare but guard anyway
        if segments.is_empty() {
            warn!("send_twitch_message called with empty text for {}", channel);
            return Ok(());
        }

        // 2) Choose credential
        let credential_opt = self
            .select_response_credential(&TwitchIRC, specified_credential_id, message_sender_user_id)
            .await?;

        let credential = match credential_opt {
            Some(c) => c,
            None => {
                let err = format!(
                    "No credential available to send Twitch IRC message to {}",
                    channel
                );
                warn!("{err}");
                return Err(Error::Internal(err));
            }
        };

        // 3) Send the FIRST chunk immediately
        info!(
            "Sending first segment ({} / {} chars) using credential {}",
            segments[0].len(),
            message.len(),
            credential.user_name
        );

        self.platform_manager
            .send_twitch_irc_message(&credential.user_name, &channel_with_hash, &segments[0])
            .await?;

        // 4) If more remain, stash them for !continue
        if segments.len() > 1 {
            let mut map = PENDING_CONTINUATIONS.lock();
            let q = map
                .entry(channel_with_hash.clone())
                .or_insert_with(VecDeque::new);
            for seg in segments.into_iter().skip(1) {
                q.push_back(seg);
            }
            info!(
                "Queued {} additional segment(s) for channel {}",
                q.len(),
                channel_with_hash
            );
        }

        Ok(())
    }
    
    /// Send a Twitch message with AI response including source handling
    pub async fn send_ai_response_to_twitch(
        &self,
        channel: &str,
        message: &str,
        raw_response: Option<&Value>,
        specified_credential_id: Option<Uuid>,
        message_sender_user_id: Uuid,
    ) -> Result<(), Error> {
        // Extract sources if raw response is provided
        let sources = if let Some(response) = raw_response {
            self.extract_sources(response)
        } else {
            Vec::new()
        };
        
        // Split the message into segments if it's too long
        let segments = self.split_message(message);
        let has_multiple_segments = segments.len() > 1;
        
        // Prepare channel name with hash
        let channel_with_hash = if !channel.starts_with('#') {
            format!("#{}", channel)
        } else {
            channel.to_string()
        };
        
        // Also store the channel name without hash for alternative lookup
        let channel_without_hash = channel.trim_start_matches('#').to_string();
        
        debug!("Setting up AI response context for channel: {} (with hash: {}, without hash: {})",
               channel, channel_with_hash, channel_without_hash);
        
        // Find a credential to send the message
        let credential_opt = self.select_response_credential(&TwitchIRC, specified_credential_id, message_sender_user_id).await?;
        
        if let Some(credential) = credential_opt {
            // Create or update the message context
            {
                let context = MessageContext {
                    full_message: message.to_string(),
                    channel: channel_with_hash.clone(),
                    segments: segments.clone(),
                    current_segment: 0,
                    sources: sources.clone(),
                    credential_id: specified_credential_id,
                    timestamp: std::time::SystemTime::now(),
                };
                
                // Store the context atomically with both channel name formats
                let mut contexts = LAST_MESSAGES.lock();
                
                // Store with hash
                contexts.insert(channel_with_hash.clone(), context.clone());
                
                // Also store without hash for robustness
                if !channel_without_hash.is_empty() && channel_without_hash != channel_with_hash {
                    let mut alt_context = context;
                    alt_context.channel = channel_without_hash.clone();
                    contexts.insert(channel_without_hash.clone(), alt_context);
                    debug!("Stored message context under both formats: {} and {}", 
                           channel_with_hash, channel_without_hash);
                }
            }
            
            // Send only the first segment
            self.platform_manager.send_twitch_irc_message(
                &credential.user_name,
                &channel_with_hash,
                &segments[0]
            ).await?;
            
            // Add appropriate hints based on what's available
            if has_multiple_segments && !sources.is_empty() {
                // Both continuation and sources are available
                self.platform_manager.send_twitch_irc_message(
                    &credential.user_name,
                    &channel_with_hash,
                    "Type !continue for more text or !sources to see the sources"
                ).await?;
            } else if has_multiple_segments {
                // Only continuation is available
                self.platform_manager.send_twitch_irc_message(
                    &credential.user_name,
                    &channel_with_hash,
                    "Type !continue to see more of the message"
                ).await?;
            } else if !sources.is_empty() {
                // Only sources are available
                self.platform_manager.send_twitch_irc_message(
                    &credential.user_name,
                    &channel_with_hash,
                    "Type !sources to see the sources for this information"
                ).await?;
            }
            
            Ok(())
        } else {
            let err_msg = format!("No credential available to send AI response to {}", channel);
            warn!("{}", err_msg);
            Err(Error::Internal(err_msg))
        }
    }

    /// Send a response consisting of multiple message lines
    pub async fn send_response(
        &self,
        response: &MessageResponse,
        message_sender_user_id: Uuid,
    ) -> Result<(), Error> {
        match response.platform.as_str() {
            "twitch-irc" => {
                for text in &response.texts {
                    if let Err(e) = self.send_twitch_message(
                        &response.channel,
                        text,
                        response.respond_credential_id,
                        message_sender_user_id
                    ).await {
                        warn!("Error sending message: {:?}", e);
                    }
                }
                Ok(())
            },
            // Add more platforms as needed
            _ => {
                Err(Error::Internal(format!(
                    "Platform '{}' not supported for message sending",
                    response.platform
                )))
            }
        }
    }

    fn split_into_chunks(text: &str, max_len: usize) -> Vec<String> {
        if text.len() <= max_len {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut current = String::new();

        for word in text.split_whitespace() {
            // +1 for the space we'll add (unless first word)
            let added_len = if current.is_empty() {
                word.len()
            } else {
                word.len() + 1
            };

            if current.len() + added_len > max_len {
                if !current.is_empty() {
                    chunks.push(current.clone());
                    current.clear();
                }
            }

            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }
}
