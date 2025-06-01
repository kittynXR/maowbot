# MaowBot UI Architecture Refactor Proposal

## Executive Summary

This proposal outlines a unified UI architecture that maintains the TUI as a development/fallback option while establishing a consistent API across GUI, overlay, and TUI implementations. The refactor aims to stabilize the UI API, improve code reuse, and enable the TUI to run within GUI tabs.

## Current State Analysis

### Architecture Overview
- **maowbot-common-ui**: Shared state, events, and gRPC client
- **maowbot-tui**: Standalone text interface with direct BotApi access
- **maowbot-gui**: Desktop application using egui
- **maowbot-overlay**: VR overlay using ImGui/OpenVR

### Key Issues
1. TUI operates independently with direct BotApi access while GUI/overlay use gRPC
2. No unified rendering abstraction across UI implementations
3. State management is duplicated between TUI and common-ui
4. TUI cannot be embedded in GUI tabs

## Proposed Architecture

### 1. Unified UI Trait System

```rust
// In maowbot-common-ui/src/traits.rs
pub trait UIBackend: Send + Sync {
    type Renderer: UIRenderer;
    
    async fn initialize(&mut self) -> Result<()>;
    async fn create_renderer(&self) -> Result<Self::Renderer>;
    fn supports_embedding(&self) -> bool;
}

pub trait UIRenderer: Send + Sync {
    async fn render_frame(&mut self, state: &AppState) -> Result<()>;
    async fn handle_input(&mut self) -> Result<Option<UIEvent>>;
    fn get_render_target(&self) -> RenderTarget;
}

pub enum RenderTarget {
    Terminal,
    Window(WindowHandle),
    Texture(TextureHandle),
    Overlay(OverlayHandle),
}

// Extended UIEvent to support TUI-specific events
pub enum UIEvent {
    // Existing events...
    Command(String),           // TUI command input
    ChatModeToggle(Platform),  // TUI chat mode switch
    TabSwitch(String),        // Tab navigation
}
```

### 2. TUI Backend Refactor

```rust
// maowbot-tui/src/backend.rs
pub struct TuiBackend {
    bot_api: Arc<dyn BotApi>,
    event_bus: Arc<EventBus>,
    render_mode: TuiRenderMode,
}

pub enum TuiRenderMode {
    Standalone,              // Direct terminal output
    Embedded(TextureHandle), // Render to texture for GUI embedding
}

impl UIBackend for TuiBackend {
    type Renderer = TuiRenderer;
    
    fn supports_embedding(&self) -> bool { true }
    
    async fn create_renderer(&self) -> Result<TuiRenderer> {
        match self.render_mode {
            TuiRenderMode::Standalone => TuiRenderer::new_terminal(),
            TuiRenderMode::Embedded(handle) => TuiRenderer::new_texture(handle),
        }
    }
}
```

### 3. Unified State Management

```rust
// maowbot-common-ui/src/state.rs
pub struct UnifiedAppState {
    // Common state
    pub chat_state: Arc<Mutex<ChatState>>,
    pub connection_status: Arc<Mutex<ConnectionStatus>>,
    pub settings: Arc<Mutex<AppSettings>>,
    
    // Platform-specific states
    pub platform_states: Arc<Mutex<HashMap<String, PlatformState>>>,
    
    // UI-specific states
    pub ui_states: Arc<Mutex<HashMap<String, Box<dyn Any + Send + Sync>>>>,
}

pub trait PlatformState: Send + Sync {
    fn platform_id(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}

// TUI-specific state that can be accessed when needed
pub struct TuiState {
    pub active_account: Option<String>,
    pub chat_mode: Option<ChatMode>,
    pub command_history: Vec<String>,
}
```

### 4. Communication Layer

```rust
// maowbot-common-ui/src/communication.rs
pub enum CommunicationMode {
    Direct(Arc<dyn BotApi>),  // For TUI in development mode
    Grpc(SharedGrpcClient),   // For production GUI/overlay
}

pub struct UnifiedClient {
    mode: CommunicationMode,
    event_tx: Sender<AppEvent>,
}

impl UnifiedClient {
    pub async fn send_command(&self, cmd: Command) -> Result<()> {
        match &self.mode {
            CommunicationMode::Direct(api) => {
                // Direct API call
                api.execute_command(cmd).await
            }
            CommunicationMode::Grpc(client) => {
                // gRPC call
                client.send_command(cmd).await
            }
        }
    }
}
```

### 5. GUI Tab Integration

```rust
// maowbot-gui/src/tabs/tui_tab.rs
pub struct TuiTab {
    backend: TuiBackend,
    renderer: Option<TuiRenderer>,
    terminal_texture: TextureHandle,
}

impl Tab for TuiTab {
    fn title(&self) -> &str { "Console" }
    
    fn render(&mut self, ui: &mut egui::Ui, state: &AppState) {
        // Create terminal-like area
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(egui::Image::new(&self.terminal_texture));
            
            // Input field at bottom
            let response = ui.text_edit_singleline(&mut self.input_buffer);
            if response.lost_focus() && ui.input().key_pressed(egui::Key::Enter) {
                self.renderer.handle_command(&self.input_buffer);
                self.input_buffer.clear();
            }
        });
    }
}
```

## Implementation Plan

### Phase 1: Core Infrastructure (Week 1-2)
1. Define unified traits in maowbot-common-ui
2. Implement UnifiedAppState with migration utilities
3. Create CommunicationMode abstraction
4. Add RenderTarget variants

### Phase 2: TUI Refactor (Week 2-3)
1. Split TUI into backend/renderer components
2. Implement texture rendering mode
3. Migrate TUI state to UnifiedAppState
4. Add UIBackend trait implementation

### Phase 3: GUI Integration (Week 3-4)
1. Create TuiTab component
2. Implement terminal emulation in egui
3. Add tab management for Console view
4. Test embedded TUI functionality

### Phase 4: API Stabilization (Week 4-5)
1. Document all public APIs
2. Create migration guide
3. Add comprehensive tests
4. Performance optimization

## Benefits

1. **Unified Experience**: Consistent behavior across all UI modes
2. **Code Reuse**: Shared state and communication logic
3. **Flexibility**: TUI available both standalone and embedded
4. **Maintainability**: Single source of truth for UI state
5. **Development**: TUI remains available for debugging/development

## Migration Strategy

1. Implement new architecture alongside existing code
2. Create adapter layers for backward compatibility
3. Migrate one UI at a time (TUI → GUI → Overlay)
4. Deprecate old APIs with clear migration path
5. Remove legacy code after full migration

## API Stability Guarantees

### Stable APIs (v1.0)
- Core UIBackend/UIRenderer traits
- UnifiedAppState structure
- Common UIEvent types
- CommunicationMode interface

### Experimental APIs
- Platform-specific state extensions
- Custom renderer implementations
- Advanced embedding features

## Testing Strategy

1. Unit tests for each backend implementation
2. Integration tests for state synchronization
3. E2E tests for embedded TUI in GUI
4. Performance benchmarks for render paths
5. Cross-platform compatibility tests

## Conclusion

This refactor provides a solid foundation for MaowBot's UI layer while maintaining flexibility for different use cases. The TUI remains available for development and debugging while gaining the ability to run within the GUI, creating a more cohesive user experience.