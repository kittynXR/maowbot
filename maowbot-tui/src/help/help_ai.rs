// File: maowbot-tui/src/help/help_ai.rs
//
// Detailed help text for the "ai" command group.

pub const AI_HELP_TEXT: &str = r#"AI Command:

Usage:

  ai
    Shows usage for the AI command (this text).

  ai enable
    Enables AI processing globally for the bot.

  ai disable
    Disables AI processing globally for the bot.

  ai status
    Shows current AI service status including enabled state, active provider,
    and statistics.

  ai provider list
    Lists all configured AI providers and their status.

  ai provider show [NAME]
    Shows configured API keys (masked) for all providers or a specific provider.

  ai configure openai --api-key <KEY> [--model <MODEL>] [--api-base <URL>]
    Configures the OpenAI provider with the specified API key and optional model.
    Default model: gpt-4

  ai configure anthropic --api-key <KEY> [--model <MODEL>]
    Configures the Anthropic provider with the specified API key and optional model.
    Default model: claude-3-opus-20240229

  ai chat <MESSAGE>
    Sends a direct message to the configured AI provider and returns the response.

Examples:
  ai enable                                       # Enable AI processing
  ai disable                                      # Disable AI processing
  ai status                                       # Show current AI status
  ai provider list                                # List all providers
  ai provider show                                # Show all API keys (masked)
  ai provider show openai                         # Show OpenAI API key info
  ai configure openai --api-key sk-...            # Configure OpenAI
  ai configure anthropic --api-key sk-ant-...     # Configure Anthropic
  ai chat "Hello, how are you?"                  # Test chat

Notes:
  - API keys are stored securely and only shown masked (last 4 chars visible)
  - You must configure at least one provider before using AI features
  - The AI service must be enabled for AI features to work
  - Custom API bases are useful for using OpenAI-compatible providers
"#;