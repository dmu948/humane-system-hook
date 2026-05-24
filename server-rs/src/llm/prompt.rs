use rig::completion::message::Message;

use super::request::LlmChatRequest;

/// Builds the provider-neutral prompt payload for a chat request.
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build_chat_history(request: &LlmChatRequest) -> Vec<Message> {
        let mut history = Vec::with_capacity(request.history.len() + 1);

        let system_prompt = request.system_prompt.trim();
        if !system_prompt.is_empty() {
            history.push(Message::system(system_prompt));
        }

        history.extend(request.history.iter().cloned());
        history
    }
}
