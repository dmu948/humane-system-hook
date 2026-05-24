use std::sync::Arc;

use reqwest::Client as HttpClient;
use tracing::info;

use crate::config::{LlmConfig, LlmProvider};

use super::backend::LlmBackend;
use super::providers;
use super::request::LlmChatRequest;

/// Hot-swappable LLM agent facade backed by an object-safe provider adapter.
pub struct LlmAgent {
    backend: Arc<dyn LlmBackend>,
}

impl LlmAgent {
    /// Build an `LlmAgent` from the loaded config.
    pub fn from_config(
        config: &LlmConfig,
        http_client: HttpClient,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let provider = config.provider;
        info!(provider = %provider, model = %config.model, "constructing LLM agent");

        let backend: Arc<dyn LlmBackend> = match provider {
            LlmProvider::Echo => providers::EchoProvider::build(),
            LlmProvider::Gemini => providers::GeminiProvider::build(config, http_client)?,
            LlmProvider::Anthropic => providers::AnthropicProvider::build(config, http_client)?,
            LlmProvider::OpenAi | LlmProvider::OpenAiCompatible => {
                providers::OpenAiProvider::build(config, http_client)?
            }
        };

        Ok(Self { backend })
    }

    /// Send a prompt with request-scoped conversation and system prompt context to the LLM.
    pub async fn chat(&self, request: LlmChatRequest) -> Result<String, String> {
        self.backend.chat(request).await
    }

    /// Send a vision prompt with an image (base64-encoded JPEG) and a question.
    pub async fn vision_prompt(
        &self,
        question: &str,
        image_base64: &str,
    ) -> Result<String, String> {
        self.backend.vision_prompt(question, image_base64).await
    }
}
