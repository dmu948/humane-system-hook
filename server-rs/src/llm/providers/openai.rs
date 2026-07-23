use std::env;
use std::sync::Arc;

use reqwest::Client as HttpClient;
use rig::completion::message::ToolChoice;
use rig::providers;
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::config::{LlmProvider, ResolvedConfig};
use crate::llm::backend::LlmBackend;
use crate::llm::memory::MemoryService;
use crate::llm::request_log::LlmRequestLogger;
use crate::llm::rig_backend::RigBackend;

pub struct OpenAiProvider;

const DEFAULT_WEB_SEARCH_CONTEXT_SIZE: &str = "low";
const VALID_WEB_SEARCH_CONTEXT_SIZES: &[&str] = &["low", "medium", "high"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct WebSearchConfig {
    enabled: bool,
    context_size: String,
}

impl WebSearchConfig {
    fn from_env() -> Self {
        Self::from_values(
            env::var("PENUMBRA_ENABLE_WEB_SEARCH").ok().as_deref(),
            env::var("PENUMBRA_WEB_SEARCH_CONTEXT_SIZE").ok().as_deref(),
        )
    }

    fn from_values(enabled: Option<&str>, context_size: Option<&str>) -> Self {
        let enabled = enabled == Some("1");
        let raw_context_size = context_size
            .unwrap_or(DEFAULT_WEB_SEARCH_CONTEXT_SIZE)
            .trim()
            .to_lowercase();
        let context_size = if VALID_WEB_SEARCH_CONTEXT_SIZES.contains(&raw_context_size.as_str()) {
            raw_context_size
        } else {
            warn!(
                value = %raw_context_size,
                "Invalid PENUMBRA_WEB_SEARCH_CONTEXT_SIZE; falling back to low"
            );
            DEFAULT_WEB_SEARCH_CONTEXT_SIZE.to_string()
        };
        Self {
            enabled,
            context_size,
        }
    }

    fn request_params(&self) -> Option<Value> {
        self.enabled.then(|| {
            json!({
                "tools": [{
                    "type": "web_search",
                    "search_context_size": self.context_size,
                }]
            })
        })
    }
}

impl OpenAiProvider {
    pub async fn build(
        config: &ResolvedConfig,
        http_client: HttpClient,
        request_logger: LlmRequestLogger,
        memory: Option<MemoryService>,
    ) -> Result<Arc<dyn LlmBackend>, Box<dyn std::error::Error + Send + Sync>> {
        let llm_config = &config.config.llm;
        let api_key = llm_config.resolve_api_key().ok_or(
            "OpenAI api_key not set; configure OPENAI_API_KEY in the environment or .env, or set llm.api_key in config.toml",
        )?;
        let mut builder = providers::openai::CompletionsClient::builder()
            .api_key(&api_key)
            .http_client(http_client.clone());
        if let Some(ref base_url) = llm_config.base_url {
            builder = builder.base_url(&normalized_base_url(&llm_config.provider, base_url));
        }
        let client = builder.build()?;

        if llm_config.provider == LlmProvider::Perplexity {
            info!(
                "Perplexity Agent API ready (model={}, endpoint=/v1/responses)",
                llm_config.model
            );
            info!("Perplexity hosted web_search enabled");
            return RigBackend::from_client_with_response_metadata(
                "Perplexity",
                client.responses_api(),
                request_logger,
                config,
                http_client,
                memory,
                |response| {
                    serde_json::to_value(response)
                        .map(|value| extract_web_citations(&value))
                        .unwrap_or_default()
                },
                |builder| {
                    builder
                        .additional_params(perplexity_request_params())
                        .tool_choice(ToolChoice::Auto)
                },
            )
            .await;
        }

        let web_search = WebSearchConfig::from_env();
        info!(
            "OpenAI agent ready (model={}, custom_base={})",
            llm_config.model,
            llm_config.base_url.is_some()
        );
        info!(
            enabled = web_search.enabled,
            context_size = web_search
                .enabled
                .then_some(web_search.context_size.as_str()),
            "OpenAI hosted web_search"
        );

        if let Some(request_params) = web_search.request_params() {
            return RigBackend::from_client_with_response_metadata(
                "OpenAI",
                client.responses_api(),
                request_logger,
                config,
                http_client,
                memory,
                |response| {
                    serde_json::to_value(response)
                        .map(|value| extract_web_citations(&value))
                        .unwrap_or_default()
                },
                move |builder| {
                    builder
                        .additional_params(request_params.clone())
                        .tool_choice(ToolChoice::Auto)
                },
            )
            .await;
        }

        RigBackend::from_client(
            "OpenAI",
            client,
            request_logger,
            config,
            http_client,
            memory,
            |builder| builder,
        )
        .await
    }
}

fn perplexity_request_params() -> Value {
    // Perplexity can otherwise echo an empty string for this field. Rig's
    // OpenAI Responses decoder correctly accepts only `auto` or `disabled`.
    // Hosted tools supplied here are merged with Rig's local function tools.
    json!({
        "truncation": "disabled",
        "tools": [{"type": "web_search"}]
    })
}

fn normalized_base_url(provider: &LlmProvider, base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if provider == &LlmProvider::Perplexity {
        // Routed Perplexity models use the Agent API. rig's Responses client
        // appends `/responses`, so retain (or add) the `/v1` API prefix.
        if trimmed.ends_with("/v1") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/v1")
        }
    } else {
        trimmed.to_string()
    }
}

fn extract_web_citations(response: &Value) -> Vec<Value> {
    response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter_map(|content| content.get("annotations").and_then(Value::as_array))
        .flatten()
        .filter(|annotation| annotation.get("type").and_then(Value::as_str) == Some("url_citation"))
        .map(|annotation| {
            json!({
                "title": annotation.get("title").cloned().unwrap_or(Value::Null),
                "url": annotation.get("url").cloned().unwrap_or(Value::Null),
                "start_index": annotation.get("start_index").cloned().unwrap_or(Value::Null),
                "end_index": annotation.get("end_index").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perplexity_base_url_targets_v1_responses_api() {
        assert_eq!(
            normalized_base_url(&LlmProvider::Perplexity, "https://api.perplexity.ai/v1/"),
            "https://api.perplexity.ai/v1"
        );
        assert_eq!(
            normalized_base_url(&LlmProvider::Perplexity, "https://api.perplexity.ai"),
            "https://api.perplexity.ai/v1"
        );
        assert_eq!(
            normalized_base_url(&LlmProvider::OpenAiCompatible, "https://example.test/v1/"),
            "https://example.test/v1"
        );
    }

    #[test]
    fn perplexity_requests_enable_hosted_search_and_decodable_truncation() {
        assert_eq!(
            perplexity_request_params(),
            json!({
                "truncation": "disabled",
                "tools": [{"type": "web_search"}]
            })
        );
    }

    #[test]
    fn web_search_is_disabled_when_unset_or_zero() {
        for enabled in [None, Some("0")] {
            let config = WebSearchConfig::from_values(enabled, None);
            assert!(!config.enabled);
            assert_eq!(config.request_params(), None);
        }
    }

    #[test]
    fn web_search_defaults_to_low_and_sets_request_fields() {
        let config = WebSearchConfig::from_values(Some("1"), None);
        assert_eq!(
            config.request_params(),
            Some(json!({
                "tools": [{
                    "type": "web_search",
                    "search_context_size": "low",
                }]
            }))
        );
    }

    #[test]
    fn web_search_accepts_medium_and_high_context_sizes() {
        for context_size in ["medium", "high"] {
            let config = WebSearchConfig::from_values(Some("1"), Some(context_size));
            assert_eq!(config.context_size, context_size);
        }
    }

    #[test]
    fn invalid_web_search_context_size_falls_back_to_low() {
        let config = WebSearchConfig::from_values(Some("1"), Some("huge"));
        assert_eq!(config.context_size, "low");
    }

    #[test]
    fn citation_extraction_returns_empty_without_annotations() {
        assert!(extract_web_citations(&json!({
            "output": [{"type": "message", "content": [{"type": "output_text"}]}]
        }))
        .is_empty());
    }

    #[test]
    fn citation_extraction_keeps_url_fields_and_ignores_other_annotations() {
        let citations = extract_web_citations(&json!({
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "annotations": [
                        {
                            "type": "url_citation",
                            "title": "Example",
                            "url": "https://example.com",
                            "start_index": 4,
                            "end_index": 11
                        },
                        {"type": "file_citation", "title": "Ignored"}
                    ]
                }]
            }]
        }));
        assert_eq!(
            citations,
            vec![json!({
                "title": "Example",
                "url": "https://example.com",
                "start_index": 4,
                "end_index": 11
            })]
        );
    }
}
