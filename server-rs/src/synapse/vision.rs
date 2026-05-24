use crate::proto::aibus::*;

/// Check if the current Understand request is a vision request.
pub fn is_vision_request(ctx: &SynapseDeviceContext) -> bool {
    for turn in ctx.turns.iter().rev() {
        if let Some(synapse_chat_turn::Content::UserRequest(req)) = &turn.content {
            return req.vision_requested
                == synapse_user_request_content::VisionRequested::Vision as i32;
        }
    }
    false
}

/// Extract the observation text from a completed UnderstandScene round-trip.
pub fn extract_vision_observation(ctx: &SynapseDeviceContext) -> Option<String> {
    let mut candidate: Option<String> = None;
    for turn in ctx.turns.iter().rev() {
        match &turn.content {
            Some(synapse_chat_turn::Content::Observation(obs)) => {
                if candidate.is_none() && !obs.is_final && !obs.observation.trim().is_empty() {
                    candidate = Some(obs.observation.trim().to_string());
                }
            }
            Some(synapse_chat_turn::Content::Action(action)) => {
                if action.action == "UnderstandScene" && candidate.is_some() {
                    return candidate;
                }
            }
            // Anything before the last UserRequest belongs to a previous conversation run and should be ignored
            Some(synapse_chat_turn::Content::UserRequest(_)) => break,
            _ => {}
        }
    }
    None
}
