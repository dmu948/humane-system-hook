use std::sync::Arc;

use crate::llm::backend::{LlmBackend, LlmFuture};
use crate::llm::request::LlmChatRequest;

pub struct EchoProvider;

impl EchoProvider {
    pub fn build() -> Arc<dyn LlmBackend> {
        tracing::info!("using echo backend (no LLM API calls)");
        Arc::new(EchoBackend)
    }
}

struct EchoBackend;

impl LlmBackend for EchoBackend {
    fn chat<'a>(&'a self, request: LlmChatRequest) -> LlmFuture<'a> {
        Box::pin(async move { Ok(format!("Echo: {}", request.utterance)) })
    }

    fn vision_prompt<'a>(&'a self, question: &'a str, _image_base64: &'a str) -> LlmFuture<'a> {
        Box::pin(async move { Ok(format!("Echo: [vision] {}", question)) })
    }
}
