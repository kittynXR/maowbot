use std::collections::VecDeque;

pub struct ChatState {
    messages: VecDeque<ChatMessage>,
    input_buffer: String,
    max_messages: usize,
}

#[repr(C)]
pub struct ChatMessage {
    pub author: [u8; 64],
    pub text: [u8; 256],
}

#[derive(Clone)]
pub struct ChatEvent {
    pub channel: String,
    pub author: String,
    pub body: String,
}

pub enum ChatCommand {
    SendMessage(String),
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            input_buffer: String::with_capacity(256),
            max_messages: 200,
        }
    }

    pub fn add_message(&mut self, event: ChatEvent) {
        let mut msg = ChatMessage {
            author: [0; 64],
            text: [0; 256],
        };

        // Copy author
        let author_bytes = event.author.as_bytes();
        let len = author_bytes.len().min(63);
        msg.author[..len].copy_from_slice(&author_bytes[..len]);

        // Copy text
        let text_bytes = event.body.as_bytes();
        let len = text_bytes.len().min(255);
        msg.text[..len].copy_from_slice(&text_bytes[..len]);

        self.messages.push_back(msg);

        // Trim old messages
        while self.messages.len() > self.max_messages {
            self.messages.pop_front();
        }
    }

    pub fn get_messages_ptr(&self) -> *const u8 {
        if self.messages.is_empty() {
            std::ptr::null()
        } else {
            self.messages.as_slices().0.as_ptr() as *const u8
        }
    }

    pub fn get_messages_count(&self) -> usize {
        self.messages.len()
    }

    pub fn get_input_buffer_ptr(&self) -> *const u8 {
        self.input_buffer.as_ptr()
    }

    pub fn get_input_buffer_ptr_mut(&mut self) -> *mut u8 {
        self.input_buffer.as_mut_ptr()
    }

    pub fn get_input_buffer_capacity(&self) -> usize {
        self.input_buffer.capacity()
    }
}