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

/// NEW: Global citation queue for `!sources`
static PENDING_SOURCES: Lazy<Mutex<HashMap<String, VecDeque<Vec<String>>>>> =
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

    
    /// Handles the !continue command - sends the next segment of a truncated message
    pub async fn handle_continue_command(
        &self,
        channel: &str,
        respond_credential_id: Option<Uuid>,
        user_id: Uuid,
    ) -> Result<bool, Error> {
        // 1) grab next queued chunk **without awaiting**
        let next_chunk_opt = {
            let mut map = PENDING_CONTINUATIONS.lock();
            if let Some(dq) = map.get_mut(channel) {
                dq.pop_front()
            } else {
                None
            }
        };

        // 2) nothing queued?
        let chunk = match next_chunk_opt {
            Some(c) => c,
            None => return Ok(false),
        };

        // 3) send it (may await) – no lock is held here
        self.send_twitch_message(channel, &chunk, respond_credential_id, user_id)
            .await
            .ok();
        Ok(true)
    }

    /*───────────────────────────────────────────────────────────*/
    /* !sources                                                  */
    /*───────────────────────────────────────────────────────────*/
    pub async fn handle_sources_command(
        &self,
        channel: &str,
        respond_credential_id: Option<Uuid>,
        user_id: Uuid,
    ) -> Result<bool, Error> {
        // 1) pull the *entire list* of citations while the lock is held
        let sources_opt = {
            let mut map = PENDING_SOURCES.lock();
            if let Some(dq) = map.get_mut(channel) {
                dq.pop_front()
            } else {
                None
            }
        };

        // 2) nothing?
        let sources = match sources_opt {
            Some(s) => s,
            None => return Ok(false),
        };

        // 3) send each citation – lock already released
        for line in sources {
            self.send_twitch_message(channel, &line, respond_credential_id, user_id)
                .await
                .ok();
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
        text: &str,
        raw_json: Option<&serde_json::Value>,
        respond_credential_id: Option<Uuid>,
        user_id: Uuid,
    ) -> Result<(), Error> {
        /* 1) Split text into ≤500‑char pieces (sync) */
        const MAX: usize = 500;
        let mut pieces = VecDeque::new();
        if text.len() > MAX {
            let mut start = 0;
            while start < text.len() {
                let end   = (start + MAX).min(text.len());
                let split = text[start..end]
                    .rfind(' ')
                    .map(|i| start + i)
                    .unwrap_or(end);
                pieces.push_back(text[start..split].trim().to_string());
                start = split;
            }
        } else {
            pieces.push_back(text.to_string());
        }

        /* 2) Send first piece immediately (await) */
        if let Some(first) = pieces.pop_front() {
            self.send_twitch_message(channel, &first, respond_credential_id, user_id)
                .await?;
        }

        /* 3) 100 % synchronous queue updates — NO await after this point */

        /* 3a) queue remaining chunks for !continue */
        if !pieces.is_empty() {
            let mut cont_q = PENDING_CONTINUATIONS.lock();
            cont_q
                .entry(channel.to_string())
                .or_default()
                .extend(pieces); // VecDeque<String>
        }

        /* 3b) queue sources for !sources */
        if let Some(raw) = raw_json {
            let sources = extract_sources(raw);
            if !sources.is_empty() {
                PENDING_SOURCES
                    .lock()
                    .entry(channel.to_string())
                    .or_default()
                    .push_back(sources); // Vec<String>
            }
        }

        Ok(())
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

fn extract_sources(raw: &serde_json::Value) -> Vec<String> {
    if let Some(arr) = raw.get("sources").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|s| {
                Some(format!(
                    "{} — {}",
                    s.get("title")?.as_str()?,
                    s.get("url")?.as_str()?
                ))
            })
            .collect();
    }
    if let Some(arr) = raw.get("annotations").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|a| {
                let c = a.get("url_citation")?;
                Some(format!(
                    "{} — {}",
                    c.get("title")?.as_str()?,
                    c.get("url")?.as_str()?
                ))
            })
            .collect();
    }
    Vec::new()
}