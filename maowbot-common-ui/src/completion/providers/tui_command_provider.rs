// TUI command completion provider
use crate::completion::{CompletionProvider, CompletionItem, CompletionCategory, CompletionContext, CompletionScope};
use async_trait::async_trait;

pub struct TuiCommandCompletionProvider {
    commands: Vec<CommandInfo>,
}

struct CommandInfo {
    name: String,
    subcommands: Vec<String>,
    description: String,
}

impl TuiCommandCompletionProvider {
    pub fn new() -> Self {
        Self {
            commands: Self::build_command_tree(),
        }
    }
    
    fn build_command_tree() -> Vec<CommandInfo> {
        vec![
            // Core Commands
            CommandInfo {
                name: "help".to_string(),
                subcommands: vec![],
                description: "Show help for commands".to_string(),
            },
            CommandInfo {
                name: "status".to_string(),
                subcommands: vec!["config".to_string()],
                description: "Show system status".to_string(),
            },
            CommandInfo {
                name: "list".to_string(),
                subcommands: vec![],
                description: "List all plugins".to_string(),
            },
            CommandInfo {
                name: "quit".to_string(),
                subcommands: vec![],
                description: "Exit the TUI".to_string(),
            },
            
            // User Management
            CommandInfo {
                name: "user".to_string(),
                subcommands: vec![
                    "add", "remove", "edit", "info", "search", "list",
                    "chat", "note", "merge", "roles", "analysis"
                ].into_iter().map(String::from).collect(),
                description: "User management".to_string(),
            },
            CommandInfo {
                name: "credential".to_string(),
                subcommands: vec![
                    "list", "refresh", "revoke", "health", "batch-refresh"
                ].into_iter().map(String::from).collect(),
                description: "Credential management".to_string(),
            },
            
            // Platform Management
            CommandInfo {
                name: "platform".to_string(),
                subcommands: vec!["add", "remove", "list", "show"].into_iter().map(String::from).collect(),
                description: "Platform configuration".to_string(),
            },
            CommandInfo {
                name: "account".to_string(),
                subcommands: vec!["add", "remove", "list", "show", "refresh"].into_iter().map(String::from).collect(),
                description: "Account management".to_string(),
            },
            CommandInfo {
                name: "connection".to_string(),
                subcommands: vec!["start", "stop", "status", "autostart", "chat"].into_iter().map(String::from).collect(),
                description: "Connection management".to_string(),
            },
            
            // Content Management
            CommandInfo {
                name: "command".to_string(),
                subcommands: vec![
                    "list", "setcooldown", "setwarnonce", "setrespond", "enable", "disable"
                ].into_iter().map(String::from).collect(),
                description: "Command management".to_string(),
            },
            CommandInfo {
                name: "redeem".to_string(),
                subcommands: vec![
                    "list", "create", "delete", "cost", "enable", "disable"
                ].into_iter().map(String::from).collect(),
                description: "Redeem management".to_string(),
            },
            CommandInfo {
                name: "config".to_string(),
                subcommands: vec![
                    "list", "get", "set", "delete", "export", "import"
                ].into_iter().map(String::from).collect(),
                description: "Configuration management".to_string(),
            },
            
            // Platform-Specific
            CommandInfo {
                name: "twitch".to_string(),
                subcommands: vec![
                    "active", "join", "part", "msg", "chat", "default"
                ].into_iter().map(String::from).collect(),
                description: "Twitch-specific commands".to_string(),
            },
            CommandInfo {
                name: "vrchat".to_string(),
                subcommands: vec!["world", "avatar", "instance"].into_iter().map(String::from).collect(),
                description: "VRChat integration".to_string(),
            },
            CommandInfo {
                name: "drip".to_string(),
                subcommands: vec!["set", "list", "fit", "props"].into_iter().map(String::from).collect(),
                description: "VRChat avatar parameters".to_string(),
            },
            
            // System & Development
            CommandInfo {
                name: "plugin".to_string(),
                subcommands: vec!["enable", "disable", "remove"].into_iter().map(String::from).collect(),
                description: "Plugin management".to_string(),
            },
            CommandInfo {
                name: "ai".to_string(),
                subcommands: vec![
                    "enable", "disable", "status", "openai", "anthropic", "chat",
                    "addtrigger", "removetrigger", "listtriggers", "systemprompt"
                ].into_iter().map(String::from).collect(),
                description: "AI configuration".to_string(),
            },
            CommandInfo {
                name: "diagnostics".to_string(),
                subcommands: vec!["health", "status", "metrics", "logs", "test"].into_iter().map(String::from).collect(),
                description: "System diagnostics".to_string(),
            },
            CommandInfo {
                name: "diag".to_string(), // Alias
                subcommands: vec!["health", "status", "metrics", "logs", "test"].into_iter().map(String::from).collect(),
                description: "System diagnostics (alias)".to_string(),
            },
            CommandInfo {
                name: "system".to_string(),
                subcommands: vec!["server", "overlay"].into_iter().map(String::from).collect(),
                description: "Process management".to_string(),
            },
        ]
    }
}

#[async_trait]
impl CompletionProvider for TuiCommandCompletionProvider {
    fn name(&self) -> &str {
        "tui_commands"
    }
    
    fn is_applicable(&self, context: &CompletionContext) -> bool {
        matches!(&context.scope, CompletionScope::TuiCommand | CompletionScope::GuiCommand)
    }
    
    async fn provide_completions(
        &self,
        context: &CompletionContext,
        prefix: &str,
    ) -> Result<Vec<CompletionItem>, Box<dyn std::error::Error + Send + Sync>> {
        let mut items = Vec::new();
        let words = context.previous_words();
        
        match words.len() {
            0 => {
                // Complete command names
                for cmd in &self.commands {
                    if cmd.name.starts_with(prefix) {
                        items.push(CompletionItem {
                            replacement: cmd.name.clone(),
                            display: cmd.name.clone(),
                            description: Some(cmd.description.clone()),
                            category: CompletionCategory::Command,
                            icon: Some("⚡".to_string()),
                            priority: 100,
                            metadata: Default::default(),
                        });
                    }
                }
            }
            _ => {
                // Complete subcommands
                let command = words[0];
                if let Some(cmd_info) = self.commands.iter().find(|c| c.name == command) {
                    for sub in &cmd_info.subcommands {
                        if sub.starts_with(prefix) {
                            items.push(CompletionItem {
                                replacement: sub.clone(),
                                display: sub.clone(),
                                description: None,
                                category: CompletionCategory::Subcommand,
                                icon: Some("▸".to_string()),
                                priority: 90,
                                metadata: Default::default(),
                            });
                        }
                    }
                }
                
                // Special case for help command
                if command == "help" && words.len() == 1 {
                    for cmd in &self.commands {
                        if cmd.name != "help" && cmd.name.starts_with(prefix) {
                            items.push(CompletionItem {
                                replacement: cmd.name.clone(),
                                display: cmd.name.clone(),
                                description: Some(cmd.description.clone()),
                                category: CompletionCategory::Argument,
                                icon: Some("?".to_string()),
                                priority: 85,
                                metadata: Default::default(),
                            });
                        }
                    }
                }
            }
        }
        
        Ok(items)
    }
}