pub fn help_pipeline() -> String {
    r#"
PIPELINE - Event Pipeline Management

  Event pipelines allow you to create database-driven rules that process
  incoming events (chat messages, follows, etc.) through filters and actions.

COMMANDS:
  pipeline list [all]                   - List pipelines (include 'all' to show disabled)
  pipeline create <name> [description]  - Create a new pipeline
                  [priority]            
                  [stop_on_match]       
                  [stop_on_error]       
  pipeline delete <id>                  - Delete a pipeline
  pipeline toggle <id> <enabled|disabled> - Enable or disable a pipeline
  pipeline show <id>                    - Show pipeline details with filters and actions
  pipeline reload                       - Reload all pipelines from database

FILTER COMMANDS:
  pipeline filter add <pipeline_id> <filter_type> [config_json] [order] [negated] [required]
    - Add a filter to a pipeline
  
  pipeline filter remove <filter_id>
    - Remove a filter from a pipeline
  
  pipeline filter list <pipeline_id>
    - List all filters for a pipeline
  
  pipeline filter types
    - Show available filter types

ACTION COMMANDS:
  pipeline action add <pipeline_id> <action_type> [config_json] [order] [continue_on_error] 
                      [is_async] [timeout_ms] [retry_count] [retry_delay_ms]
    - Add an action to a pipeline
  
  pipeline action remove <action_id>
    - Remove an action from a pipeline
  
  pipeline action list <pipeline_id>
    - List all actions for a pipeline
  
  pipeline action types
    - Show available action types

HISTORY COMMANDS:
  pipeline history [pipeline_id] [limit] [offset]
    - Show execution history (optionally filtered by pipeline)

EXAMPLES:
  # Create a pipeline for welcoming new users
  pipeline create "Welcome Message" "Welcomes new chatters" 100 true false
  
  # Add a filter to check if it's a chat message event
  pipeline filter add <pipeline_id> "event_type_filter" "{\"event_types\": [\"chat.message\"]}"
  
  # Add a filter to check if user is new
  pipeline filter add <pipeline_id> "user_first_message" "{\"within_minutes\": 60}"
  
  # Add an action to send a welcome message
  pipeline action add <pipeline_id> "twitch_message" "{\"message_template\": \"Welcome {{user.display_name}} to the stream!\"}"
  
  # Enable the pipeline
  pipeline toggle <pipeline_id> enabled
  
  # View execution history
  pipeline history <pipeline_id> 20

NOTES:
  - Pipelines are processed in priority order (lower numbers first)
  - If 'stop_on_match' is true, no further pipelines will process the event
  - Filters are evaluated in order; all must pass for actions to execute
  - Actions are executed in order unless continue_on_error is true
  - Configuration is passed as JSON strings for filters and actions
"#.to_string()
}