use std::collections::HashMap;
use std::time::{Duration, Instant};

use std::sync::{Arc, Mutex};

use base64::Engine as _;
use reqwest::Client as HttpClient;
use rig::agent::{Agent, AgentBuilder, HookAction, PromptHook};
use rig::client::CompletionClient;
use rig::completion::message::{AssistantContent, ImageMediaType, Message, UserContent};
use rig::completion::CompletionModel;
use rig::completion::Prompt;
use rig::completion::{CompletionResponse, PromptError};
use rig::tool::Tool;
use rig::OneOrMany;
use tracing::{error, info, warn};

use crate::config::ResolvedConfig;
use crate::llm::ChatResult;

use super::backend::{LlmBackend, LlmFuture};
use super::error::friendly_error_message;
use super::memory::MemoryService;
use super::model_router::{is_weather_request, routed_models};
use super::prompt::PromptBuilder;
use super::request::LlmChatRequest;
use super::request_log::LlmRequestLogger;
use super::tools::registry::LlmToolContext;
use super::tools::understand_scene::UnderstandSceneTool;

/// Marker for a termination due to device vision request
const DEFERRED_VISION_SENTINEL: &str = "__HUMANE_DEFERRED_VISION__";

/// Rig hook to prevent execution of the `understand_scene` tool. The returned termination value
/// is used to trigger a DeferredVision response to the client
struct CompletionHook<R> {
    response_metadata_extractor: Arc<dyn Fn(&R) -> Vec<serde_json::Value> + Send + Sync>,
    response_metadata: Arc<Mutex<Vec<serde_json::Value>>>,
    weather_result: Arc<Mutex<Option<String>>>,
}

impl<R> Clone for CompletionHook<R> {
    fn clone(&self) -> Self {
        Self {
            response_metadata_extractor: Arc::clone(&self.response_metadata_extractor),
            response_metadata: Arc::clone(&self.response_metadata),
            weather_result: Arc::clone(&self.weather_result),
        }
    }
}

impl<M> PromptHook<M> for CompletionHook<M::Response>
where
    M: CompletionModel,
{
    async fn on_completion_response(
        &self,
        _prompt: &Message,
        response: &CompletionResponse<M::Response>,
    ) -> HookAction {
        let metadata = (self.response_metadata_extractor)(&response.raw_response);
        if !metadata.is_empty() {
            if let Ok(mut response_metadata) = self.response_metadata.lock() {
                response_metadata.extend(metadata);
            }
        }
        let selected_vision = response.choice.iter().any(|content| {
            matches!(
                content,
                AssistantContent::ToolCall(call)
                    if call.function.name == UnderstandSceneTool::NAME
            )
        });

        if selected_vision {
            HookAction::terminate(DEFERRED_VISION_SENTINEL)
        } else {
            HookAction::cont()
        }
    }

    async fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        _args: &str,
        result: &str,
    ) -> HookAction {
        if tool_name == "weather_get" {
            if let Ok(mut weather_result) = self.weather_result.lock() {
                *weather_result = Some(result.to_string());
            }
        }
        HookAction::cont()
    }
}

/// Shared LLM backend for providers
pub struct RigBackend<M>
where
    M: CompletionModel + 'static,
    (): PromptHook<M> + 'static,
{
    provider_label: &'static str,
    agents: HashMap<String, Agent<M>>,
    default_model: String,
    request_logger: LlmRequestLogger,
    max_tool_turns: usize,
    tool_concurrency: usize,
    response_metadata_extractor: Arc<dyn Fn(&M::Response) -> Vec<serde_json::Value> + Send + Sync>,
}

impl<M> RigBackend<M>
where
    M: CompletionModel + 'static,
    (): PromptHook<M> + 'static,
{
    pub async fn from_client<C, F>(
        provider_label: &'static str,
        client: C,
        request_logger: LlmRequestLogger,
        config: &ResolvedConfig,
        http_client: HttpClient,
        memory: Option<MemoryService>,
        customize_builder: F,
    ) -> Result<Arc<dyn LlmBackend>, Box<dyn std::error::Error + Send + Sync>>
    where
        C: CompletionClient<CompletionModel = M> + Clone,
        F: Fn(AgentBuilder<M>) -> AgentBuilder<M>,
    {
        Self::from_client_with_response_metadata(
            provider_label,
            client,
            request_logger,
            config,
            http_client,
            memory,
            |_| Vec::new(),
            customize_builder,
        )
        .await
    }

    pub async fn from_client_with_response_metadata<C, F, E>(
        provider_label: &'static str,
        client: C,
        request_logger: LlmRequestLogger,
        config: &ResolvedConfig,
        http_client: HttpClient,
        memory: Option<MemoryService>,
        response_metadata_extractor: E,
        customize_builder: F,
    ) -> Result<Arc<dyn LlmBackend>, Box<dyn std::error::Error + Send + Sync>>
    where
        C: CompletionClient<CompletionModel = M> + Clone,
        F: Fn(AgentBuilder<M>) -> AgentBuilder<M>,
        E: Fn(&M::Response) -> Vec<serde_json::Value> + Send + Sync + 'static,
    {
        let llm_config = &config.config.llm;
        let is_perplexity = llm_config
            .base_url
            .as_deref()
            .is_some_and(|url| url.contains("api.perplexity.ai"));
        let models = if is_perplexity {
            routed_models().into_iter().collect::<Vec<_>>()
        } else {
            vec![llm_config.model.as_str()]
        };
        let tool_context = LlmToolContext::new(http_client, config, memory);
        let mut agents = HashMap::new();
        for model in models {
            let builder = customize_builder(client.clone().agent(model));
            let tool_resources = if llm_config.tools.enabled {
                tool_context
                    .build_tool_resources(llm_config)
                    .await
                    .map_err(|err| -> Box<dyn std::error::Error + Send + Sync> {
                        std::io::Error::new(std::io::ErrorKind::Other, err).into()
                    })?
            } else {
                None
            };
            let agent = match tool_resources {
                Some(resources) => resources.apply(builder).build(),
                None => builder.build(),
            };
            agents.insert(model.to_string(), agent);
        }
        if is_perplexity {
            info!("intelligent model router ready");
        }

        Ok(Arc::new(Self {
            provider_label,
            agents,
            default_model: if is_perplexity {
                super::model_router::TERRA_MODEL.to_string()
            } else {
                llm_config.model.clone()
            },
            request_logger,
            max_tool_turns: llm_config.tools.max_tool_turns,
            tool_concurrency: llm_config.tools.tool_concurrency,
            response_metadata_extractor: Arc::new(response_metadata_extractor),
        }))
    }
}

fn weather_fallback_text(raw: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(raw).ok()?;
    if let Some(encoded) = value.as_str() {
        value = serde_json::from_str(encoded).ok()?;
    }
    let response = value.get("response").unwrap_or(&value);
    if response.get("success").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    let data = response.get("data")?;
    let location = json_text(data, "nearest_area").or_else(|| json_text(data, "location"));
    let condition = json_text(data, "condition");
    let temperature =
        json_text(data, "temperature_f").map(|value| format!("{value} degrees Fahrenheit"));
    let feels_like = json_text(data, "feels_like_f").map(|value| format!("feels like {value}"));
    let humidity = json_text(data, "humidity").map(|value| format!("humidity {value} percent"));
    let wind = match (
        json_text(data, "wind_direction"),
        json_text(data, "wind_mph"),
    ) {
        (Some(direction), Some(speed)) => {
            Some(format!("wind {direction} at {speed} miles per hour"))
        }
        (None, Some(speed)) => Some(format!("wind {speed} miles per hour")),
        _ => None,
    };

    let details = [condition, temperature, feels_like, humidity, wind]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if details.is_empty() {
        return None;
    }
    Some(match location {
        Some(location) => format!("Current weather for {location}: {}.", details.join(", ")),
        None => format!("Current weather: {}.", details.join(", ")),
    })
}

fn json_text(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::weather_fallback_text;

    #[test]
    fn successful_native_weather_result_has_spoken_fallback() {
        let raw = r#"{"response":{"success":true,"data":{"location":"Fairfax","nearest_area":"Fairfax","condition":"Partly cloudy","temperature_f":"72","feels_like_f":"73","humidity":"48","wind_direction":"NW","wind_mph":"6"}}}"#;
        assert_eq!(
            weather_fallback_text(raw).as_deref(),
            Some("Current weather for Fairfax: Partly cloudy, 72 degrees Fahrenheit, feels like 73, humidity 48 percent, wind NW at 6 miles per hour.")
        );
    }

    #[test]
    fn failed_or_empty_weather_result_does_not_mask_timeout() {
        assert_eq!(
            weather_fallback_text(r#"{"response":{"success":false}}"#),
            None
        );
        assert_eq!(
            weather_fallback_text(r#"{"response":{"success":true,"data":{}}}"#),
            None
        );
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
            let run_id = request.template_context.run_id.clone();
            let history = PromptBuilder::build_chat_history(&request);
            let started = Instant::now();

            let content = if let Some(image_bytes) = &request.image {
                OneOrMany::many(vec![
                    UserContent::text(utterance.clone()),
                    UserContent::image_base64(
                        &base64::engine::general_purpose::STANDARD.encode(image_bytes),
                        Some(ImageMediaType::JPEG),
                        None,
                    ),
                ])
                .expect("non-empty content vec")
            } else {
                OneOrMany::one(UserContent::text(utterance.clone()))
            };

            let user_message = Message::User { content };
            let response_metadata = Arc::new(Mutex::new(Vec::new()));
            let weather_result = Arc::new(Mutex::new(None));
            let requested_model = request.model.as_deref().unwrap_or(&self.default_model);
            let (model, agent) = self
                .agents
                .get_key_value(requested_model)
                .or_else(|| self.agents.get_key_value(&self.default_model))
                .expect("Rig backend always has a default agent");

            let prompt = agent
                .prompt(user_message)
                .with_history(history.clone())
                .max_turns(self.max_tool_turns)
                .with_tool_concurrency(self.tool_concurrency.max(1))
                .with_hook(CompletionHook {
                    response_metadata_extractor: Arc::clone(&self.response_metadata_extractor),
                    response_metadata: Arc::clone(&response_metadata),
                    weather_result: Arc::clone(&weather_result),
                });
            let raw_result = tokio::time::timeout(Duration::from_secs(22), prompt).await;
            let latency_ms = started.elapsed().as_millis();
            let response_metadata = response_metadata
                .lock()
                .map(|metadata| metadata.clone())
                .unwrap_or_default();
            if !response_metadata.is_empty() {
                info!(
                    provider = self.provider_label,
                    web_citations = ?response_metadata,
                    "LLM response includes web citations"
                );
            }

            let result = match raw_result {
                Ok(Ok(text)) => Ok(ChatResult::Text(text)),
                Ok(Err(PromptError::PromptCancelled { reason, .. }))
                    if reason == DEFERRED_VISION_SENTINEL =>
                {
                    Ok(ChatResult::DeferredVision)
                }
                Ok(Err(e)) => {
                    error!(provider = self.provider_label, error = %e, "LLM chat failed");
                    Err(friendly_error_message(&e))
                }
                Err(_) => {
                    let fallback = weather_result.lock().ok().and_then(|result| result.clone());
                    if is_weather_request(&utterance) {
                        if let Some(result) = fallback.and_then(|raw| weather_fallback_text(&raw)) {
                            warn!(model = %model, "LLM post-tool call timed out; using weather tool result fallback");
                            Ok(ChatResult::Text(result))
                        } else {
                            Err(
                                "The weather request timed out before I could finish the response."
                                    .into(),
                            )
                        }
                    } else {
                        Err("The request took too long to complete. Please try again.".into())
                    }
                }
            };

            self.request_logger
                .log_chat(
                    self.provider_label,
                    &run_id,
                    &history,
                    &utterance,
                    match &result {
                        Ok(ChatResult::Text(text)) => Some(text.as_str()),
                        Ok(ChatResult::DeferredVision) => None,
                        Err(_) => None,
                    },
                    result.clone().err().as_deref(),
                    latency_ms,
                )
                .await;

            result
        })
    }
}
