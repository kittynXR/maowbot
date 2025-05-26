use std::collections::VecDeque;

#[derive(Clone)]
pub struct ChatMessage {
    pub author: String,
    pub text: String,
}

#[derive(Clone)]
pub struct ChatEvent {
    pub channel: String,
    pub author: String,
    pub body: String,
}

pub struct ChatState {
    messages: VecDeque<ChatMessage>,
    max_messages: usize,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            max_messages: 200,
        }
    }

    pub fn add_message(&mut self, event: ChatEvent) {
        let msg = ChatMessage {
            author: event.author,
            text: event.body,
        };

        self.messages.push_back(msg);

        // Trim old messages
        while self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
    }

    pub fn messages(&self) -> &VecDeque<ChatMessage> {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

// For C FFI (used by overlay)
#[repr(C)]
pub struct ChatMessageFFI {
    pub author: [u8; 64],
    pub text: [u8; 256],
}

impl ChatState {
    pub fn to_ffi_messages(&self) -> Vec<ChatMessageFFI> {
        self.messages
            .iter()
            .map(|msg| {
                let mut ffi_msg = ChatMessageFFI {
                    author: [0; 64],
                    text: [0; 256],
                };

                let author_bytes = msg.author.as_bytes();
                let len = author_bytes.len().min(63);
                ffi_msg.author[..len].copy_from_slice(&author_bytes[..len]);

                let text_bytes = msg.text.as_bytes();
                let len = text_bytes.len().min(255);
                ffi_msg.text[..len].copy_from_slice(&text_bytes[..len]);

                ffi_msg
            })
            .collect()
    }
}