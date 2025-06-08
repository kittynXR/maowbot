use crate::services::event_pipeline::{PipelineBuilder, EventPipeline};
use maowbot_common::models::platform::Platform;

/// Example pipeline configurations demonstrating various use cases

/// Stream announcement pipeline - Twitch stream goes live, notify Discord
pub fn create_stream_announcement_pipeline() -> EventPipeline {
    PipelineBuilder::new("stream_announce", "Stream Announcement")
        .priority(10)
        .platform(vec![Platform::TwitchEventSub])
        .log(tracing::Level::INFO)
        .discord_message(
            "bot",
            "123456789", // announcements channel
            "🔴 **{broadcaster}** is now live on Twitch!\n\n{title}\nPlaying: {game}\n\nhttps://twitch.tv/{broadcaster}"
        )
        .osc_trigger("/avatar/parameters/streaming", 1.0, None)
        .build()
}

/// Chat command pipeline - Process !commands in chat
pub fn create_chat_command_pipeline() -> EventPipeline {
    PipelineBuilder::new("chat_commands", "Chat Commands")
        .priority(50)
        .platform(vec![Platform::TwitchIRC, Platform::Discord])
        .message_pattern(vec![r"^!"], false).unwrap()
        .plugin_action("command_processor", "handle_command", |p| {
            p.param("prefix", "!")
        })
        .build()
}

/// Raid notification pipeline - Thank raiders and trigger effects
pub fn create_raid_pipeline() -> EventPipeline {
    PipelineBuilder::new("raid_thanks", "Raid Thank You")
        .priority(20)
        .platform(vec![Platform::TwitchEventSub])
        // Filter for raid events (would need proper event type filtering)
        .discord_message(
            "bot",
            "987654321", // community channel
            "🎉 **{raider}** just raided with {viewers} viewers! Thank you!"
        )
        .osc_trigger("/avatar/parameters/happy", 1.0, Some(10000))
        .osc_trigger("/avatar/parameters/confetti", 1.0, Some(5000))
        .build()
}

/// Moderation pipeline - Auto-timeout certain patterns
pub fn create_moderation_pipeline() -> EventPipeline {
    PipelineBuilder::new("auto_mod", "Auto Moderation")
        .priority(1) // Very high priority
        .stop_on_match(true) // Don't process other pipelines for moderated messages
        .platform(vec![Platform::TwitchIRC])
        .message_pattern(vec![
            r"(?i)buy.+followers",
            r"(?i)bit\.ly",
            r"(?i)discord\.gg",
        ], true).unwrap() // Match any pattern
        .plugin_action("moderation", "timeout_user", |p| {
            p.param("duration", "600") // 10 minutes
             .param("reason", "Automated: Suspicious link")
        })
        .log(tracing::Level::WARN)
        .build()
}

/// VIP greeting pipeline - Special welcome for VIPs
pub fn create_vip_greeting_pipeline() -> EventPipeline {
    PipelineBuilder::new("vip_greet", "VIP Greeting")
        .priority(30)
        .platform(vec![Platform::TwitchIRC])
        .user_roles(vec!["vip", "moderator"], true) // VIP or Moderator
        // Would need a "first message in session" filter
        .discord_message(
            "bot",
            "123456789",
            "✨ VIP **{user}** has arrived! ✨"
        )
        .osc_trigger("/avatar/parameters/vip_alert", 1.0, Some(3000))
        .build()
}

/// Time-based pipeline - Different behavior based on time
pub fn create_late_night_pipeline() -> EventPipeline {
    PipelineBuilder::new("late_night", "Late Night Mode")
        .priority(60)
        .platform(vec![Platform::TwitchIRC, Platform::Discord])
        .time_window(22, 6, chrono_tz::US::Eastern) // 10 PM to 6 AM EST
        .message_pattern(vec![r"^!"], false).unwrap()
        .plugin_action("late_night_commands", "handle", |p| {
            p.param("mode", "quiet")
        })
        .build()
}

/// Cross-platform mirror pipeline - Mirror specific Discord channel to Twitch
pub fn create_discord_mirror_pipeline() -> EventPipeline {
    PipelineBuilder::new("discord_mirror", "Discord to Twitch Mirror")
        .priority(70)
        .platform(vec![Platform::Discord])
        .channel(vec!["stream-chat"])
        .plugin_action("cross_platform", "mirror_message", |p| {
            p.param("target_platform", "twitch")
             .param("format", "[Discord] {user}: {message}")
        })
        .build()
}

/// Subscription celebration pipeline
pub fn create_sub_celebration_pipeline() -> EventPipeline {
    PipelineBuilder::new("sub_celebrate", "Subscription Celebration")
        .priority(15)
        .platform(vec![Platform::TwitchEventSub])
        // Would filter for subscription events
        .composite_filter(false, |f| { // Celebrate new subs OR gift subs
            f // Add specific event type filters here
        })
        .discord_message(
            "bot",
            "123456789",
            "🎊 **{user}** just subscribed! (Tier {tier}) 🎊\n{message}"
        )
        .osc_trigger("/avatar/parameters/celebrate", 1.0, Some(5000))
        .plugin_action("alerts", "play_sound", |p| {
            p.param("sound", "sub_alert.mp3")
             .param("volume", "0.8")
        })
        .build()
}

/// AI response pipeline - Respond to questions with AI
pub fn create_ai_response_pipeline() -> EventPipeline {
    PipelineBuilder::new("ai_respond", "AI Auto-Response")
        .priority(80)
        .platform(vec![Platform::TwitchIRC, Platform::Discord])
        .message_pattern(vec![r"^(?:hey |hi |hello )?bot[,:]?\s+(.+)"], false).unwrap()
        .plugin_action("ai_service", "generate_response", |p| {
            p.param("model", "claude-3")
             .param("max_tokens", "150")
             .param("temperature", "0.7")
        })
        .build()
}

/// Load all example pipelines into a vector
pub fn load_example_pipelines() -> Vec<EventPipeline> {
    vec![
        create_stream_announcement_pipeline(),
        create_chat_command_pipeline(),
        create_raid_pipeline(),
        create_moderation_pipeline(),
        create_vip_greeting_pipeline(),
        create_late_night_pipeline(),
        create_discord_mirror_pipeline(),
        create_sub_celebration_pipeline(),
        create_ai_response_pipeline(),
    ]
}