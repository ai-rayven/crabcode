use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String
}

#[derive(Serialize)]
pub struct OllamaRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool
}

#[derive(Deserialize)]
pub struct OllamaResponse {
    pub message: Option<Message>,
    pub done: bool
}

#[derive(Default)]
pub struct AppState {
    pub input_buffer: String,
    pub chat_history: Vec<Message>,
    pub scroll_offset: u16
}

#[derive(Debug, Clone)]
pub enum Action {
    Response(String),
    Stream(String),
    Done
}

#[derive(Clone)]
pub enum AgentCommand {
    Run(Vec<Message>)
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolCall {
    ReadFile(String),
    None
}
