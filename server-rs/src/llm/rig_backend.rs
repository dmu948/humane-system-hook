use std::sync::Arc;

use rig::agent::{Agent, PromptHook};
use rig::completion::CompletionModel;
use rig::completion::{Chat, Prompt};
use tracing::error;

use super::backend::{LlmBackend, LlmFuture};
use super::error::friendly_error_message;
use super::prompt::PromptBuilder;
use super::providers::vision_message;
use super::request::LlmChatRequest;

/// Shared LLM backend for providers
pub struct RigBackend<M>
where
    M: CompletionModel + 'static,
    (): PromptHook<M> + 'static,
{
    provider_label: &'static str,
    agent: Agent<M>,
}

impl<M> RigBackend<M>
where
    M: CompletionModel + 'static,
    (): PromptHook<M> + 'static,
{
    pub fn new(provider_label: &'static str, agent: Agent<M>) -> Self {
        Self {
            provider_label,
            agent,
        }
    }

    pub fn arc(provider_label: &'static str, agent: Agent<M>) -> Arc<dyn LlmBackend> {
        Arc::new(Self::new(provider_label, agent))
    }
}

impl<M> LlmBackend for RigBackend<M>
where
    M: CompletionModel + 'static,
    (): PromptHook<M> + 'static,
{
    fn chat<'a>(&'a self, request: LlmChatRequest) -> LlmFuture<'a> {
        Box::pin(async move {
            let utterance = request.utterance.clone();
            let history = PromptBuilder::build_chat_history(&request);

            self.agent.chat(utterance, history).await.map_err(|e| {
                error!(provider = self.provider_label, error = %e, "LLM chat failed");
                friendly_error_message(&e)
            })
        })
    }

    fn vision_prompt<'a>(&'a self, question: &'a str, image_base64: &'a str) -> LlmFuture<'a> {
        Box::pin(async move {
            self.agent
                .prompt(vision_message(question, image_base64))
                .await
                .map_err(|e| {
                    error!(provider = self.provider_label, error = %e, "LLM vision prompt failed");
                    friendly_error_message(&e)
                })
        })
    }
}
