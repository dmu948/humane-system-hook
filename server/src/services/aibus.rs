use std::pin::Pin;
use std::sync::Arc;

use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{info, warn};
use uuid::Uuid;

use crate::llm::LlmAgent;
use crate::proto::aibus::*;
use crate::proto::aibus::ai_bus_service_server::AiBusService;

pub struct AiBusServiceImpl {
    pub agent: Arc<LlmAgent>,
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

        // Log device context if present
        if let Some(ref ctx) = req.device_context {
            info!(
                turns = ctx.turns.len(),
                is_locked = ctx.is_locked,
                "    device_context"
            );
        }

        if let Some(ref loc) = req.location {
            info!(
                lat = loc.latitude,
                lon = loc.longitude,
                "    location"
            );
        }

        // Call LLM agent
        let response_text = match self.agent.prompt(utterance).await {
            Ok(text) => text,
            Err(e) => {
                warn!(error = %e, "LLM prompt failed, falling back to error message");
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
