// File: maowbot-tui/src/help/help_ai.rs
//
// Detailed help text for the "ai" command group.

pub const AI_HELP_TEXT: &str = r#"AI Command:

Usage:

  ai
    Shows usage for the AI command (this text).

  ai enable
    Enables AI processing for trigger prefixes.

  ai disable
    Disables AI processing for trigger prefixes.

  ai status
    Shows current AI configuration including enabled status and providers.

  ai openai --api-key <KEY> [--model <MODEL>] [--api-base <URL>]
    Configures the OpenAI provider with the specified API key and optional model.
    Default model: gpt-4

  ai anthropic --api-key <KEY> [--model <MODEL>] [--api-base <URL>]
    Configures the Anthropic provider with the specified API key and optional model.
    Default model: claude-3-opus-20240229

  ai chat <MESSAGE>
    Sends a direct message to the configured AI provider and returns the response.

  ai register <FUNCTION_NAME> <DESCRIPTION>
    Registers a new function that the AI can call with the given name and description.

  ai systemprompt <PROMPT>
    Sets the system prompt used for AI interactions.

  ai addtrigger <PREFIX>
    Adds a new trigger prefix that will activate AI processing in chat.

  ai removetrigger <PREFIX>
    Removes a trigger prefix from the list of AI triggers.

  ai listtriggers
    Lists all configured trigger prefixes.

Examples:
  ai status
  ai openai --api-key sk-abcdef123456 --model gpt-4
  ai anthropic --api-key sk-ant-api123456 --model claude-3-sonnet-20240229
  ai chat What is the weather like today?
  ai systemprompt You are a helpful AI assistant for MaowBot.
  ai addtrigger !ai
  ai removetrigger hey maow
"#;