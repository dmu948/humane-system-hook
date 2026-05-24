use std::fmt::Display;

/// Convert a raw LLM provider error into a friendly, speakable sentence.
pub fn friendly_error_message(e: &impl Display) -> String {
    let raw = e.to_string().to_lowercase();

    if raw.contains("429")
        || raw.contains("rate limit")
        || raw.contains("resource_exhausted")
        || raw.contains("too many requests")
    {
        "I'm getting too many requests right now. Please try again in a moment.".into()
    } else if raw.contains("401")
        || raw.contains("403")
        || raw.contains("unauthorized")
        || raw.contains("forbidden")
        || raw.contains("invalid api key")
        || raw.contains("permission denied")
    {
        "There's a problem with the API key configuration. Please check the server settings.".into()
    } else if raw.contains("404")
        || raw.contains("model not found")
        || raw.contains("not_found")
        || raw.contains("does not exist")
    {
        "The configured AI model wasn't found. Please check the server settings.".into()
    } else if raw.contains("500")
        || raw.contains("502")
        || raw.contains("503")
        || raw.contains("internal server error")
        || raw.contains("service unavailable")
        || raw.contains("bad gateway")
    {
        "The AI service is temporarily unavailable. Please try again shortly.".into()
    } else if raw.contains("timeout")
        || raw.contains("timed out")
        || raw.contains("deadline exceeded")
    {
        "The request to the AI service timed out. Please try again.".into()
    } else if raw.contains("connection")
        || raw.contains("dns")
        || raw.contains("resolve")
        || raw.contains("unreachable")
    {
        "I couldn't reach the AI service. Please check the server's internet connection.".into()
    } else if raw.contains("content filter")
        || raw.contains("safety")
        || raw.contains("blocked")
        || raw.contains("harm_category")
    {
        "The AI service declined to answer that. Try rephrasing your question.".into()
    } else if raw.contains("context length")
        || raw.contains("too long")
        || raw.contains("max tokens")
        || raw.contains("token limit")
    {
        "That conversation got too long for the AI service to handle. Try starting a new one."
            .into()
    } else {
        "Something went wrong while contacting the AI service. Please try again.".into()
    }
}
