mod anthropic;
mod echo;
mod gemini;
mod openai;

pub use anthropic::AnthropicProvider;
pub use echo::EchoProvider;
pub use gemini::GeminiProvider;
pub use openai::OpenAiProvider;

use rig::completion::message::{ImageMediaType, Message, UserContent};
use rig::OneOrMany;

pub fn vision_message(question: &str, image_base64: &str) -> Message {
    Message::User {
        content: OneOrMany::many(vec![
            UserContent::text(question),
            UserContent::image_base64(
                image_base64,
                Some(ImageMediaType::JPEG),
                None, // detail: auto
            ),
        ])
        .expect("non-empty content vec"),
    }
}
