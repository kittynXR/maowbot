# Unified Completion System Design

## Overview
We've created a context-aware, extensible completion system in `maowbot-common-ui` that can be used across all UI components (TUI, GUI, overlay) and supports dynamic data sources.

## Architecture

### Core Components

1. **CompletionEngine** - The main orchestrator
   - Manages multiple providers
   - Handles caching
   - Applies fuzzy matching
   - Groups and sorts results

2. **CompletionContext** - Context information
   - Where: TUI, Twitch chat, Discord, overlay, etc.
   - What: Current input, cursor position
   - Who: User ID, roles, permissions
   - State: Stream online/offline, metadata

3. **CompletionProvider** - Data sources
   - **TuiCommandProvider**: TUI commands and subcommands
   - **CommandProvider**: Twitch/Discord chat commands
   - **EmoteProvider**: Twitch, 7TV, BTTV, FFZ emotes
   - **UserProvider**: Recent chatters, @mentions
   - Extensible for plugins to add their own

4. **CompletionItem** - Rich completion data
   - Text to insert
   - Display text with formatting
   - Category (command, emote, user, etc.)
   - Icon/emoji
   - Priority for sorting
   - Metadata (URLs, IDs, etc.)

## Usage Examples

### 1. TUI Command Line
```rust
// User types: "us<TAB>"
Context: TuiCommand
Result: 
  ⚡ user               User management

// User types: "user i<TAB>"
Context: TuiCommand with previous word "user"
Result:
  ▸ info
  ▸ info               (if multiple matches)
```

### 2. Twitch Chat
```rust
// User types: "!pi<TAB>"
Context: TwitchChat { channel: "kittyn" }
Result:
  ! !ping              Check if bot is responsive
  ! !pizza             Order a virtual pizza

// User types: "@kit<TAB>"
Context: TwitchChat, is_mention = true
Result:
  @ @kittyn            broadcaster, vip
  @ @kittybot          moderator

// User types: "pog<TAB>"
Context: TwitchChat, not a command
Result:
  😀 PogChamp          Twitch emote
  7️⃣ POGGERS          7TV emote
  🅱️ PogU              BTTV emote
```

### 3. GUI/Overlay
```rust
// In overlay chat, user types: ":cat<TAB>"
Context: OverlayChat, is_emote_shortcode = true
Result:
  😀 catJAM            7TV emote
  😀 catKISS           BTTV emote
  😀 CatLove           Channel emote
```

## Integration Points

### For TUI
```rust
// In main.rs
use maowbot_tui::unified_completer::UnifiedCompleter;

let mut rl = Editor::<UnifiedCompleter>::new()?;
rl.set_helper(Some(UnifiedCompleter::new(client)));
```

### For GUI (egui example)
```rust
// In GUI text input handler
if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
    let context = CompletionContext::new(
        CompletionScope::GuiCommand,
        self.input_text.clone(),
        self.cursor_pos,
    );
    
    let completions = self.completion_engine
        .get_completions(&context)
        .await;
    
    // Show completion popup
    self.show_completions_popup(completions);
}
```

### For Overlay
```cpp
// In overlay chat handler
void HandleTabKey() {
    auto context = CreateCompletionContext(
        currentChannel,
        inputBuffer,
        cursorPos
    );
    
    auto completions = grpcClient->GetCompletions(context);
    ShowCompletionOverlay(completions);
}
```

## Provider Implementation

### Adding a New Provider
```rust
pub struct PluginCommandProvider {
    plugin_name: String,
}

#[async_trait]
impl CompletionProvider for PluginCommandProvider {
    fn name(&self) -> &str {
        &self.plugin_name
    }
    
    fn is_applicable(&self, context: &CompletionContext) -> bool {
        // Only in chat contexts for plugin commands
        context.is_command() && 
        context.command_name() == Some(&self.plugin_name)
    }
    
    async fn provide_completions(
        &self,
        context: &CompletionContext,
        prefix: &str,
    ) -> Result<Vec<CompletionItem>> {
        // Return plugin-specific completions
    }
}
```

## Features

1. **Context Awareness**
   - Different completions for different contexts
   - Permission-based filtering
   - Stream state awareness

2. **Rich Data**
   - Icons and categories
   - Descriptions and metadata
   - Priority-based sorting

3. **Performance**
   - Async providers
   - Intelligent caching
   - Fuzzy matching

4. **Extensibility**
   - Plugins can register providers
   - Custom categories
   - Metadata support

## Future Enhancements

1. **Learning System**
   - Track usage frequency
   - Personalized suggestions
   - Context-based predictions

2. **Rich Display**
   - Emote previews
   - Command help tooltips
   - Inline documentation

3. **Multi-source Integration**
   - Global emotes API
   - User badges/roles
   - Channel-specific data

4. **Advanced Features**
   - Multi-word completion
   - Parameter hints
   - Snippet expansion

## Benefits

1. **Unified Experience** - Same completion behavior everywhere
2. **Discoverable** - Users can explore available options
3. **Efficient** - Reduces typing and errors
4. **Extensible** - Easy to add new data sources
5. **Contextual** - Right suggestions at the right time