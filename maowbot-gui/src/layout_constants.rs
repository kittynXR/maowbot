// Layout constants for consistent spacing across the UI

pub const HEADER_HEIGHT: f32 = 25.0;  // Height of section headers
pub const SEPARATOR_HEIGHT: f32 = 5.0; // Height of horizontal separators
pub const INPUT_HEIGHT: f32 = 30.0;    // Height of input areas
pub const INPUT_PADDING: f32 = 10.0;   // Padding around input areas
pub const CONTENT_MARGIN: f32 = 10.0;   // Margin for content areas
pub const VERTICAL_CONTAINER_PADDING: f32 = 5.0; // Padding added by ui.vertical()
pub const GROUP_WIDGET_MARGIN: f32 = 7.0; // Total margin added by ui.group()

// Calculate total non-chat height for consistent chat area sizing
pub const CHAT_CHROME_HEIGHT: f32 = HEADER_HEIGHT + SEPARATOR_HEIGHT + INPUT_HEIGHT + INPUT_PADDING * 2.0;