use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::Error;
use crate::eventbus::BotEvent;
use crate::services::event_pipeline::{EventAction, ActionResult, ActionContext};

#[derive(Debug, Serialize, Deserialize)]
struct AiRespondActionConfig {
    #[serde(default)]
    provider_id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    prompt_template: String,
    #[serde(default = "default_max_tokens")]
    max_tokens: u32,
    #[serde(default = "default_temperature")]
    temperature: f32,
    #[serde(default)]
    send_response: bool,
    #[serde(default)]
    response_prefix: String,
}

fn default_max_tokens() -> u32 {
    150
}

fn default_temperature() -> f32 {
    0.7
}

/// Action that generates an AI response
pub struct AiRespondAction {
    provider_id: Option<String>,
    model: Option<String>,
    system_prompt: Option<String>,
    prompt_template: String,
    max_tokens: u32,
    temperature: f32,
    send_response: bool,
    response_prefix: String,
}

impl AiRespondAction {
    pub fn new() -> Self {
        Self {
            provider_id: None,
            model: None,
            system_prompt: None,
            prompt_template: String::new(),
            max_tokens: 150,
            temperature: 0.7,
            send_response: true,
            response_prefix: String::new(),
        }
    }
    
    fn format_prompt(&self, context: &ActionContext) -> String {
        let mut prompt = self.prompt_template.clone();
        
        // Replace common placeholders
        match &context.event {
            BotEvent::ChatMessage { platform, channel, user, text, .. } => {
                prompt = prompt.replace("{platform}", platform);
                prompt = prompt.replace("{channel}", channel);
                prompt = prompt.replace("{user}", user);
                prompt = prompt.replace("{message}", text);
                prompt = prompt.replace("{text}", text);
            }
            BotEvent::TwitchEventSub(event) => {
                prompt = prompt.replace("{event_type}", &format!("{:?}", event));
            }
            _ => {}
        }
        
        // Replace shared data placeholders
        for (key, value) in &context.shared_data {
            if let Some(str_val) = value.as_str() {
                prompt = prompt.replace(&format!("{{{}}}", key), str_val);
            }
        }
        
        prompt
    }
}

impl Default for AiRespondAction {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventAction for AiRespondAction {
    fn id(&self) -> &str {
        "ai_respond"
    }

    fn name(&self) -> &str {
        "AI Respond"
    }

    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error> {
        let config: AiRespondActionConfig = serde_json::from_value(config)
            .map_err(|e| Error::Platform(format!("Invalid AI respond action config: {}", e)))?;
        
        self.provider_id = config.provider_id;
        self.model = config.model;
        self.system_prompt = config.system_prompt;
        self.prompt_template = config.prompt_template;
        self.max_tokens = config.max_tokens;
        self.temperature = config.temperature;
        self.send_response = config.send_response;
        self.response_prefix = config.response_prefix;
        Ok(())
    }

    async fn execute(&self, context: &mut ActionContext) -> Result<ActionResult, Error> {
        // TODO: Implement AI service call
        // For now, just log what we would do
        let prompt = self.format_prompt(context);
        
        tracing::info!(
            "Would generate AI response with provider={:?}, model={:?}, prompt='{}'",
            self.provider_id, self.model, prompt
        );
        
        // Simulated response
        let ai_response = format!("AI response to: {}", prompt);
        
        // Store response in shared data
        context.set_data("ai_response", serde_json::Value::String(ai_response.clone()));
        
        // Send response if configured
        if self.send_response {
            match &context.event {
                BotEvent::ChatMessage { platform, channel, .. } => {
                    let message = if self.response_prefix.is_empty() {
                        ai_response.clone()
                    } else {
                        format!("{} {}", self.response_prefix, ai_response)
                    };
                    
                    // Get account from platform
                    let account = match platform.as_str() {
                        "twitch" => "default", // TODO: Get from context
                        "discord" => "default",
                        _ => "default",
                    };
                    
                    // Send via appropriate platform
                    match platform.as_str() {
                        "twitch" => {
                            let user_id = uuid::Uuid::new_v4(); // TODO: Get proper user ID
                            context.context.message_sender
                                .send_twitch_message(
                                    channel,
                                    &message,
                                    None,
                                    user_id,
                                )
                                .await?;
                        }
                        "discord" => {
                            // TODO: Get guild ID from context
                            let guild_id = "";
                            context.context.platform_manager
                                .send_discord_message(
                                    account,
                                    guild_id,
                                    channel,
                                    &message,
                                )
                                .await?;
                        }
                        _ => {
                            return Ok(ActionResult::Error(format!("Unsupported platform: {}", platform)));
                        }
                    }
                }
                _ => {}
            }
        }
        
        Ok(ActionResult::Success(serde_json::json!({
            "ai_generated": true,
            "prompt": prompt,
            "response": ai_response,
            "sent": self.send_response,
            "provider": self.provider_id,
            "model": self.model
        })))
    }
}