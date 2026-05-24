use rig::completion::message::Message;
use tracing::debug;

use crate::proto::aibus::*;

/// Extract conversation history from device_context.turns into rig Messages.
///
/// Mapping (from GRPC_SERVICES.md):
///   user_request (USER)              -> Message::user(request text)
///   action (ASSISTANT) "Respond"     -> Message::assistant(response text from JSON)
///   message (ASSISTANT)              -> Message::assistant(content)
///   message (SYSTEM)                 -> Message::system(content)
///   Everything else                  -> skipped (internal ReAct plumbing)
pub fn extract_history(ctx: &SynapseDeviceContext) -> Vec<Message> {
    let mut history = Vec::new();

    let last_user_request_idx = ctx
        .turns
        .iter()
        .rposition(|t| matches!(&t.content, Some(synapse_chat_turn::Content::UserRequest(_))));

    for (i, turn) in ctx.turns.iter().enumerate() {
        // Skip the current run's user_request
        if Some(i) == last_user_request_idx {
            continue;
        }

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
