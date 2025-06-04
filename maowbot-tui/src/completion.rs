// Tab completion support for TUI commands
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{Validator, ValidationContext, ValidationResult};
use rustyline::{Helper, Context};
use rustyline_derive::Helper;
use std::borrow::Cow;

#[derive(Helper)]
pub struct TuiCompleter {
    commands: Vec<CommandInfo>,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
}

#[derive(Clone)]
struct CommandInfo {
    name: String,
    subcommands: Vec<String>,
    description: String,
}

impl TuiCompleter {
    pub fn new() -> Self {
        Self {
            commands: Self::build_command_tree(),
            highlighter: MatchingBracketHighlighter::new(),
            hinter: HistoryHinter::new(),
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
                    "add".to_string(),
                    "remove".to_string(),
                    "edit".to_string(),
                    "info".to_string(),
                    "search".to_string(),
                    "list".to_string(),
                    "chat".to_string(),
                    "note".to_string(),
                    "merge".to_string(),
                    "roles".to_string(),
                    "analysis".to_string(),
                ],
                description: "User management".to_string(),
            },
            CommandInfo {
                name: "credential".to_string(),
                subcommands: vec![
                    "list".to_string(),
                    "refresh".to_string(),
                    "revoke".to_string(),
                    "health".to_string(),
                    "batch-refresh".to_string(),
                ],
                description: "Credential management".to_string(),
            },
            
            // Platform Management
            CommandInfo {
                name: "platform".to_string(),
                subcommands: vec![
                    "add".to_string(),
                    "remove".to_string(),
                    "list".to_string(),
                    "show".to_string(),
                ],
                description: "Platform configuration".to_string(),
            },
            CommandInfo {
                name: "account".to_string(),
                subcommands: vec![
                    "add".to_string(),
                    "remove".to_string(),
                    "list".to_string(),
                    "show".to_string(),
                    "refresh".to_string(),
                ],
                description: "Account management".to_string(),
            },
            CommandInfo {
                name: "connection".to_string(),
                subcommands: vec![
                    "start".to_string(),
                    "stop".to_string(),
                    "status".to_string(),
                    "autostart".to_string(),
                    "chat".to_string(),
                ],
                description: "Connection management".to_string(),
            },
            
            // Content Management
            CommandInfo {
                name: "command".to_string(),
                subcommands: vec![
                    "list".to_string(),
                    "setcooldown".to_string(),
                    "setwarnonce".to_string(),
                    "setrespond".to_string(),
                    "enable".to_string(),
                    "disable".to_string(),
                ],
                description: "Command management".to_string(),
            },
            CommandInfo {
                name: "redeem".to_string(),
                subcommands: vec![
                    "list".to_string(),
                    "create".to_string(),
                    "delete".to_string(),
                    "cost".to_string(),
                    "enable".to_string(),
                    "disable".to_string(),
                ],
                description: "Redeem management".to_string(),
            },
            CommandInfo {
                name: "config".to_string(),
                subcommands: vec![
                    "list".to_string(),
                    "get".to_string(),
                    "set".to_string(),
                    "delete".to_string(),
                    "export".to_string(),
                    "import".to_string(),
                ],
                description: "Configuration management".to_string(),
            },
            
            // Platform-Specific
            CommandInfo {
                name: "twitch".to_string(),
                subcommands: vec![
                    "active".to_string(),
                    "join".to_string(),
                    "part".to_string(),
                    "msg".to_string(),
                    "chat".to_string(),
                    "default".to_string(),
                ],
                description: "Twitch-specific commands".to_string(),
            },
            CommandInfo {
                name: "vrchat".to_string(),
                subcommands: vec![
                    "world".to_string(),
                    "avatar".to_string(),
                    "instance".to_string(),
                ],
                description: "VRChat integration".to_string(),
            },
            CommandInfo {
                name: "drip".to_string(),
                subcommands: vec![
                    "set".to_string(),
                    "list".to_string(),
                    "fit".to_string(),
                    "props".to_string(),
                ],
                description: "VRChat avatar parameters".to_string(),
            },
            
            // System & Development
            CommandInfo {
                name: "plugin".to_string(),
                subcommands: vec![
                    "enable".to_string(),
                    "disable".to_string(),
                    "remove".to_string(),
                ],
                description: "Plugin management".to_string(),
            },
            CommandInfo {
                name: "ai".to_string(),
                subcommands: vec![
                    "enable".to_string(),
                    "disable".to_string(),
                    "status".to_string(),
                    "openai".to_string(),
                    "anthropic".to_string(),
                    "chat".to_string(),
                    "addtrigger".to_string(),
                    "removetrigger".to_string(),
                    "listtriggers".to_string(),
                    "systemprompt".to_string(),
                ],
                description: "AI configuration".to_string(),
            },
            CommandInfo {
                name: "diagnostics".to_string(),
                subcommands: vec![
                    "health".to_string(),
                    "status".to_string(),
                    "metrics".to_string(),
                    "logs".to_string(),
                    "test".to_string(),
                ],
                description: "System diagnostics".to_string(),
            },
            CommandInfo {
                name: "diag".to_string(), // Alias
                subcommands: vec![
                    "health".to_string(),
                    "status".to_string(),
                    "metrics".to_string(),
                    "logs".to_string(),
                    "test".to_string(),
                ],
                description: "System diagnostics (alias)".to_string(),
            },
            CommandInfo {
                name: "system".to_string(),
                subcommands: vec![
                    "server".to_string(),
                    "overlay".to_string(),
                ],
                description: "Process management".to_string(),
            },
            CommandInfo {
                name: "test_harness".to_string(),
                subcommands: vec![
                    "run-all".to_string(),
                    "twitch".to_string(),
                    "commands".to_string(),
                    "redeems".to_string(),
                    "grpc".to_string(),
                ],
                description: "Testing framework".to_string(),
            },
            CommandInfo {
                name: "simulate".to_string(),
                subcommands: vec![],
                description: "Simulate events".to_string(),
            },
            CommandInfo {
                name: "osc".to_string(),
                subcommands: vec![
                    "start".to_string(),
                    "stop".to_string(),
                    "status".to_string(),
                    "test".to_string(),
                    "toggle".to_string(),
                ],
                description: "OSC control".to_string(),
            },
        ]
    }
}

impl Completer for TuiCompleter {
    type Candidate = Pair;
    
    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let mut candidates = Vec::new();
        let line_to_cursor = &line[..pos];
        let parts: Vec<&str> = line_to_cursor.split_whitespace().collect();
        
        // Calculate where the completion should start
        let start = if line_to_cursor.ends_with(' ') {
            pos
        } else {
            line_to_cursor.rfind(' ').map(|i| i + 1).unwrap_or(0)
        };
        
        match parts.len() {
            0 => {
                // Nothing typed yet, show all commands
                for cmd in &self.commands {
                    candidates.push(Pair {
                        display: format!("{:<15} {}", cmd.name, cmd.description),
                        replacement: cmd.name.clone(),
                    });
                }
            }
            1 => {
                // Partial command typed
                let prefix = parts[0];
                for cmd in &self.commands {
                    if cmd.name.starts_with(prefix) {
                        candidates.push(Pair {
                            display: format!("{:<15} {}", cmd.name, cmd.description),
                            replacement: cmd.name.clone(),
                        });
                    }
                }
            }
            _ => {
                // Command typed, complete subcommands
                let command = parts[0];
                if let Some(cmd_info) = self.commands.iter().find(|c| c.name == command) {
                    if !cmd_info.subcommands.is_empty() {
                        let prefix = if line_to_cursor.ends_with(' ') {
                            ""
                        } else {
                            parts.last().unwrap_or(&"")
                        };
                        
                        for sub in &cmd_info.subcommands {
                            if sub.starts_with(prefix) {
                                candidates.push(Pair {
                                    display: sub.clone(),
                                    replacement: sub.clone(),
                                });
                            }
                        }
                    }
                }
                
                // Special case for help command - complete other command names
                if command == "help" && parts.len() == 2 {
                    let prefix = parts[1];
                    for cmd in &self.commands {
                        if cmd.name != "help" && cmd.name.starts_with(prefix) {
                            candidates.push(Pair {
                                display: cmd.name.clone(),
                                replacement: cmd.name.clone(),
                            });
                        }
                    }
                }
            }
        }
        
        Ok((start, candidates))
    }
}

impl Validator for TuiCompleter {
    fn validate(&self, _ctx: &mut ValidationContext) -> Result<ValidationResult, ReadlineError> {
        Ok(ValidationResult::Valid(None))
    }
}

impl Hinter for TuiCompleter {
    type Hint = String;
    
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for TuiCompleter {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }
    
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        self.highlighter.highlight_prompt(prompt, default)
    }
    
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[2m{}\x1b[0m", hint))
    }
    
    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}