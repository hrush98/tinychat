use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct Session {
    messages: Vec<ChatMessage>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.messages.clear();
    }

    pub fn push_user(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: ChatRole::User,
            content,
        });
    }

    pub fn push_assistant(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content,
        });
    }

    pub fn build_request_messages(&self, system_prompt: &str) -> Vec<ChatMessage> {
        let mut messages = Vec::with_capacity(self.messages.len() + 1);
        messages.push(ChatMessage {
            role: ChatRole::System,
            content: system_prompt.to_string(),
        });
        messages.extend(self.messages.iter().cloned());
        messages
    }
}
