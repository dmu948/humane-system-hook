use rig::completion::message::Message;

/// Request-scoped context for building an LLM call
pub struct LlmChatRequest {
    pub utterance: String,
    pub history: Vec<Message>,
    pub system_prompt: String,
}

impl LlmChatRequest {
    pub fn new(utterance: String, history: Vec<Message>, system_prompt: String) -> Self {
        Self {
            utterance,
            history,
            system_prompt,
        }
    }
}
