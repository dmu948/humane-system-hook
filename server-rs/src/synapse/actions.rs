use uuid::Uuid;

use crate::proto::aibus::*;

impl SynapseUnderstandingResponse {
    pub fn action_response(
        action_name: &str,
        thought: &str,
        input_json: &str,
        parent_id: &str,
    ) -> Self {
        let turn_id = Uuid::new_v4().to_string();

        let action = SynapseActionContent {
            thought: thought.into(),
            action: action_name.into(),
            input: input_json.into(),
            device_payload: Vec::new(),
            source: SynapseSource::Server as i32,
        };

        let turn = SynapseChatTurn {
            user: SynapseUser::Assistant as i32,
            timestamp: None,
            identifier: turn_id,
            parent_identifier: parent_id.into(),
            content: Some(synapse_chat_turn::Content::Action(action)),
        };

        Self {
            response: String::new(),
            is_final: false,
            body: Some(synapse_understanding_response::Body::Turn(turn)),
        }
    }
}
