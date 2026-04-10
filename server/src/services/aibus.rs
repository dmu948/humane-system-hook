use std::pin::Pin;
use std::sync::Arc;

use rig::completion::message::Message;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{info, warn, debug};
use uuid::Uuid;

use crate::llm::LlmAgent;
use crate::proto::aibus::*;
use crate::proto::aibus::ai_bus_service_server::AiBusService;

pub struct AiBusServiceImpl {
    pub agent: Arc<LlmAgent>,
}

/// Extract conversation history from device_context.turns into rig Messages.
///
/// Mapping (from GRPC_SERVICES.md):
///   user_request (USER)              → Message::user(request text)
///   action (ASSISTANT) "Respond"     → Message::assistant(response text from JSON)
///   message (ASSISTANT)              → Message::assistant(content)
///   message (SYSTEM)                 → Message::system(content)
///   Everything else                  → skipped (internal ReAct plumbing)
fn extract_history(ctx: &SynapseDeviceContext) -> Vec<Message> {
    let mut history = Vec::new();

    for turn in &ctx.turns {
        let user = turn.user(); // SynapseUser enum
        let content = match &turn.content {
            Some(c) => c,
            None => continue,
        };

        match content {
            synapse_chat_turn::Content::UserRequest(req) => {
                // Use repaired_request if available, otherwise the raw request
                let text = if !req.repaired_request.is_empty() {
                    &req.repaired_request
                } else {
                    &req.request
                };
                if !text.is_empty() {
                    debug!(text = %text, "  history: user_request");
                    history.push(Message::user(text));
                }
            }

            synapse_chat_turn::Content::Action(action) => {
                if action.action == "Respond" {
                    // Parse the response text from the JSON input field:
                    // {"Response": "actual text"}
                    if let Some(response_text) = extract_respond_text(&action.input) {
                        if !response_text.is_empty() {
                            debug!(text = %response_text, "  history: action(Respond)");
                            history.push(Message::assistant(response_text));
                        }
                    }
                }
                // Non-Respond actions (SearchWeb, UnderstandScene, etc.) are
                // internal ReAct tool calls — skip for LLM context.
            }

            synapse_chat_turn::Content::Message(msg) => {
                if !msg.content.is_empty() {
                    match user {
                        SynapseUser::Assistant => {
                            debug!(text = %msg.content, "  history: message(assistant)");
                            history.push(Message::assistant(&msg.content));
                        }
                        SynapseUser::System => {
                            debug!(text = %msg.content, "  history: message(system)");
                            history.push(Message::system(&msg.content));
                        }
                        _ => {
                            // USER messages as message content are unusual, treat as user
                            debug!(text = %msg.content, "  history: message(user)");
                            history.push(Message::user(&msg.content));
                        }
                    }
                }
            }

            // Observation, tao, interpretation, end, speech — skip
            _ => {}
        }
    }

    history
}

/// Parse the Response text from a Respond action's JSON input.
/// Expected format: {"Response": "some text"}
fn extract_respond_text(input: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(input).ok()?;
    parsed.get("Response")?.as_str().map(|s| s.to_string())
}

#[tonic::async_trait]
impl AiBusService for AiBusServiceImpl {
    type UnderstandStream =
        Pin<Box<dyn Stream<Item = Result<SynapseUnderstandingResponse, Status>> + Send>>;

    async fn understand(
        &self,
        request: Request<SynapseUnderstandingRequest>,
    ) -> Result<Response<Self::UnderstandStream>, Status> {
        let metadata = request.metadata().clone();
        let req = request.into_inner();

        let utterance = &req.utterance;
        let run_id = metadata
            .get("x-ai-mic-run-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        info!(run_id = %run_id, utterance = %utterance, ">>> Understand");

        // Extract conversation history from device context
        let history = if let Some(ref ctx) = req.device_context {
            info!(
                turns = ctx.turns.len(),
                is_locked = ctx.is_locked,
                location = %ctx.reverse_geocoded_location,
                "    device_context"
            );
            let h = extract_history(ctx);
            if !h.is_empty() {
                info!(messages = h.len(), "    extracted history");
            }
            h
        } else {
            Vec::new()
        };

        if let Some(ref loc) = req.location {
            info!(
                lat = loc.latitude,
                lon = loc.longitude,
                "    location"
            );
        }

        // Call LLM agent with conversation history
        let response_text = match self.agent.chat(utterance, history).await {
            Ok(text) => text,
            Err(e) => {
                warn!(error = %e, "LLM chat failed, falling back to error message");
                format!("Sorry, I encountered an error: {}", e)
            }
        };

        let turn_id = Uuid::new_v4().to_string();

        info!(response = %response_text, "<<< Understand responding");

        let action = SynapseActionContent {
            thought: "I should respond to the user".into(),
            action: "Respond".into(),
            input: serde_json::json!({"Response": response_text}).to_string(),
            device_payload: Vec::new(),
            source: SynapseSource::Server as i32,
        };

        let turn = SynapseChatTurn {
            user: SynapseUser::Assistant as i32,
            timestamp: None,
            identifier: turn_id,
            parent_identifier: run_id,
            content: Some(synapse_chat_turn::Content::Action(action)),
        };

        let response = SynapseUnderstandingResponse {
            response: String::new(),
            is_final: false,
            body: Some(synapse_understanding_response::Body::Turn(turn)),
        };

        // Stream a single response then complete
        let stream = tokio_stream::once(Ok(response));
        Ok(Response::new(Box::pin(stream)))
    }
}
