# Tab Completion Implementation Plan

## Overview
Add tab completion support to both the TUI and Twitch chat to improve user experience.

## Part 1: TUI Tab Completion

### Current State
- Uses basic `tokio::io::BufReader` for input
- No readline or completion support
- Commands are well-structured and follow consistent patterns

### Implementation Steps

#### 1. Add Rustyline Dependency
```toml
# In maowbot-tui/Cargo.toml
[dependencies]
rustyline = "14.0"
rustyline-derive = "0.10"
```

#### 2. Create Completion Helper
```rust
// maowbot-tui/src/completion.rs
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::Helper;

#[derive(Helper)]
pub struct TuiCompleter {
    commands: Vec<CommandInfo>,
}

struct CommandInfo {
    name: String,
    subcommands: Vec<String>,
    description: String,
}

impl TuiCompleter {
    pub fn new() -> Self {
        Self {
            commands: Self::build_command_tree(),
        }
    }
    
    fn build_command_tree() -> Vec<CommandInfo> {
        vec![
            CommandInfo {
                name: "user".to_string(),
                subcommands: vec!["add", "remove", "edit", "info", "search", "list", "chat", "note", "merge", "roles", "analysis"],
                description: "User management",
            },
            CommandInfo {
                name: "credential".to_string(),
                subcommands: vec!["list", "refresh", "revoke", "health", "batch-refresh"],
                description: "Credential management",
            },
            // ... more commands
        ]
    }
}

impl Completer for TuiCompleter {
    type Candidate = Pair;
    
    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let mut candidates = Vec::new();
        let parts: Vec<&str> = line[..pos].split_whitespace().collect();
        
        match parts.len() {
            0 | 1 => {
                // Complete command names
                let prefix = parts.get(0).unwrap_or(&"");
                for cmd in &self.commands {
                    if cmd.name.starts_with(prefix) {
                        candidates.push(Pair {
                            display: cmd.name.clone(),
                            replacement: cmd.name.clone(),
                        });
                    }
                }
            }
            2 => {
                // Complete subcommands
                if let Some(cmd) = self.commands.iter().find(|c| c.name == parts[0]) {
                    let prefix = parts[1];
                    for sub in &cmd.subcommands {
                        if sub.starts_with(prefix) {
                            candidates.push(Pair {
                                display: sub.clone(),
                                replacement: sub.clone(),
                            });
                        }
                    }
                }
            }
            _ => {
                // Context-specific completion (users, platforms, etc.)
                self.complete_context_specific(parts, &mut candidates);
            }
        }
        
        Ok((0, candidates))
    }
}
```

#### 3. Update Main Loop
```rust
// maowbot-tui/src/main.rs
use rustyline::Editor;
use crate::completion::TuiCompleter;

async fn run_tui_loop(client: GrpcClient, ...) -> Result<()> {
    let mut rl = Editor::<TuiCompleter>::new()?;
    rl.set_helper(Some(TuiCompleter::new()));
    
    loop {
        let prompt = tui_module.get_prompt();
        match rl.readline(&prompt) {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                let (should_quit, response) = dispatch_grpc(&line, &client, ...).await;
                // ... handle response
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("^D");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }
}
```

#### 4. Dynamic Completion Data
For context-aware completion (usernames, platform names, etc.):

```rust
impl TuiCompleter {
    async fn complete_context_specific(&self, parts: &[&str], candidates: &mut Vec<Pair>) {
        match (parts[0], parts.get(1)) {
            ("user", Some(&"info" | &"edit" | &"remove")) => {
                // Fetch cached usernames from gRPC
                if let Ok(users) = self.fetch_usernames().await {
                    let prefix = parts.get(2).unwrap_or(&"");
                    for user in users {
                        if user.starts_with(prefix) {
                            candidates.push(Pair {
                                display: user.clone(),
                                replacement: user,
                            });
                        }
                    }
                }
            }
            ("platform", Some(&"add")) => {
                // Complete platform types
                let platforms = vec!["twitch", "discord", "vrchat"];
                let prefix = parts.get(2).unwrap_or(&"");
                for platform in platforms {
                    if platform.starts_with(prefix) {
                        candidates.push(Pair {
                            display: platform.to_string(),
                            replacement: platform.to_string(),
                        });
                    }
                }
            }
            // ... more context-specific completions
        }
    }
}
```

## Part 2: Twitch Chat Tab Completion

### Current State
- Commands stored in database with platform-specific entries
- CommandService handles command lookup and execution
- No completion support in chat interface

### Implementation Steps

#### 1. Extend Command Service
```rust
// maowbot-proto/proto/services/command_service.proto
message GetCommandCompletionsRequest {
  string platform = 1;
  string channel = 2;
  string user_id = 3;
  string prefix = 4;
  bool include_descriptions = 5;
}

message GetCommandCompletionsResponse {
  repeated CommandCompletion completions = 1;
}

message CommandCompletion {
  string command = 1;
  string description = 2;
  bool available = 3; // Based on roles/stream state
}
```

#### 2. Implement Completion Logic
```rust
// maowbot-core/src/services/twitch/command_service.rs
impl CommandService {
    pub async fn get_completions(
        &self,
        platform: &str,
        channel: &str,
        user_id: &str,
        prefix: &str,
    ) -> Vec<CommandCompletion> {
        let commands = self.command_cache.read().await;
        let user_roles = self.get_user_roles(user_id).await;
        let is_online = self.is_stream_online(channel).await;
        
        commands.values()
            .filter(|cmd| {
                cmd.command_name.starts_with(prefix) &&
                cmd.platform == platform &&
                self.user_can_execute(cmd, &user_roles, is_online)
            })
            .map(|cmd| CommandCompletion {
                command: format!("!{}", cmd.command_name),
                description: cmd.response_text.chars().take(50).collect(),
                available: true,
            })
            .collect()
    }
}
```

#### 3. Overlay Integration
```cpp
// maowbot-overlay/src/chat.cpp
class ChatInputHandler {
    std::vector<std::string> completions;
    size_t completion_index = 0;
    std::string completion_prefix;
    
    void HandleTabKey(ChatState& state) {
        if (completions.empty()) {
            // Start new completion
            completion_prefix = ExtractPrefix(state.input_buffer);
            completions = RequestCompletions(completion_prefix);
            completion_index = 0;
        } else {
            // Cycle through completions
            completion_index = (completion_index + 1) % completions.size();
        }
        
        if (!completions.empty()) {
            ReplaceInput(state, completions[completion_index]);
        }
    }
    
    void RenderCompletions(ImGuiIO& io) {
        if (!completions.empty()) {
            ImGui::SetTooltip("Tab: %s (%d/%d)", 
                completions[completion_index].c_str(),
                completion_index + 1,
                completions.size());
        }
    }
};
```

#### 4. Enhanced Features

##### A. Fuzzy Matching
```rust
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

let matcher = SkimMatcherV2::default();
completions.sort_by_key(|cmd| {
    matcher.fuzzy_match(&cmd.command_name, prefix)
        .unwrap_or(0)
});
```

##### B. Usage Frequency
```rust
// Track command usage
struct CommandUsage {
    command: String,
    channel: String,
    user_id: String,
    count: i64,
    last_used: DateTime<Utc>,
}

// Sort completions by frequency
completions.sort_by(|a, b| {
    let a_usage = get_usage_count(a, channel, user_id);
    let b_usage = get_usage_count(b, channel, user_id);
    b_usage.cmp(&a_usage)
});
```

##### C. Username Completion
```rust
// Complete @mentions
if prefix.starts_with("@") {
    let recent_chatters = get_recent_chatters(channel, 100);
    for chatter in recent_chatters {
        if chatter.username.starts_with(&prefix[1..]) {
            completions.push(format!("@{}", chatter.username));
        }
    }
}
```

## Part 3: Shared Infrastructure

### 1. Completion Cache
```rust
// maowbot-common/src/completion_cache.rs
pub struct CompletionCache {
    commands: Arc<RwLock<HashMap<String, Vec<String>>>>,
    users: Arc<RwLock<HashMap<String, Vec<String>>>>,
    ttl: Duration,
}

impl CompletionCache {
    pub async fn get_or_fetch<F>(&self, key: &str, fetcher: F) -> Vec<String>
    where
        F: Future<Output = Vec<String>>,
    {
        // Check cache first
        if let Some(cached) = self.get(key).await {
            return cached;
        }
        
        // Fetch and cache
        let items = fetcher.await;
        self.set(key, items.clone()).await;
        items
    }
}
```

### 2. Configuration
```toml
# config.toml
[completion]
enabled = true
fuzzy_matching = true
max_suggestions = 10
cache_ttl_seconds = 300
include_descriptions = true
track_usage = true
```

## Implementation Priority

1. **Phase 1**: Basic TUI completion (2-3 days)
   - Add rustyline
   - Implement static command completion
   - Test with all commands

2. **Phase 2**: Dynamic TUI completion (2-3 days)
   - Add gRPC methods for fetching completion data
   - Implement context-aware completion
   - Add caching

3. **Phase 3**: Twitch chat backend (3-4 days)
   - Extend command service
   - Add completion gRPC endpoints
   - Implement filtering logic

4. **Phase 4**: Overlay integration (3-4 days)
   - Modify C++ chat input handler
   - Add completion UI
   - Test with real streams

5. **Phase 5**: Enhanced features (2-3 days)
   - Fuzzy matching
   - Usage tracking
   - Username completion

Total estimated time: 2-3 weeks for full implementation

## Testing Plan

1. **Unit Tests**
   - Completion algorithm tests
   - Permission filtering tests
   - Cache behavior tests

2. **Integration Tests**
   - TUI completion with mock gRPC
   - Command service completion tests
   - Overlay communication tests

3. **User Testing**
   - Test with streamers
   - Gather feedback on UX
   - Performance testing with many commands

## Future Enhancements

1. **Multi-word completion** - Complete command arguments
2. **Learning system** - Adapt to user preferences
3. **Plugin support** - Allow plugins to register completions
4. **Rich completions** - Show command help inline
5. **Emote completion** - Complete Twitch/Discord emotes