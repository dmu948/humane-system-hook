use std::future::Future;
use std::pin::Pin;

use super::request::LlmChatRequest;

pub type LlmFuture<'a> = Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

/// Object-safe application boundary around provider-specific LLM clients.
///
/// Rig's `Prompt` and `Chat` traits are not object-safe, so provider wrappers
/// implement this trait and keep Rig's concrete types hidden behind dynamic
/// dispatch at the app boundary.
pub trait LlmBackend: Send + Sync {
    fn chat<'a>(&'a self, request: LlmChatRequest) -> LlmFuture<'a>;

    fn vision_prompt<'a>(&'a self, question: &'a str, image_base64: &'a str) -> LlmFuture<'a>;
}
