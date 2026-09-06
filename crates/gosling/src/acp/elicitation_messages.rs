//! Typed JSON-RPC wrappers for `elicitation/create`, which the ACP crate does
//! not ship as a first-class message. Gosling both sends this request (as an
//! agent, to its clients) and receives it (as a client of an ACP provider).

use agent_client_protocol::schema::v1::{
    CreateElicitationRequest, CreateElicitationResponse, CLIENT_METHOD_NAMES,
};
use agent_client_protocol::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, UntypedMessage};

#[derive(Debug, Clone)]
pub(crate) struct CreateElicitationRequestMessage(pub(crate) CreateElicitationRequest);

impl JsonRpcMessage for CreateElicitationRequestMessage {
    fn matches_method(method: &str) -> bool {
        method == CLIENT_METHOD_NAMES.elicitation_create
    }

    fn method(&self) -> &str {
        CLIENT_METHOD_NAMES.elicitation_create
    }

    fn to_untyped_message(&self) -> Result<UntypedMessage, agent_client_protocol::Error> {
        UntypedMessage::new(CLIENT_METHOD_NAMES.elicitation_create, &self.0)
    }

    fn parse_message(
        method: &str,
        params: &impl serde::Serialize,
    ) -> Result<Self, agent_client_protocol::Error> {
        if !Self::matches_method(method) {
            return Err(agent_client_protocol::Error::method_not_found());
        }

        Ok(Self(agent_client_protocol::util::json_cast_params(params)?))
    }
}

impl JsonRpcRequest for CreateElicitationRequestMessage {
    type Response = CreateElicitationResponseMessage;
}

#[derive(Debug, Clone)]
pub(crate) struct CreateElicitationResponseMessage(pub(crate) CreateElicitationResponse);

impl JsonRpcResponse for CreateElicitationResponseMessage {
    fn into_json(self, _method: &str) -> Result<serde_json::Value, agent_client_protocol::Error> {
        serde_json::to_value(self.0).map_err(agent_client_protocol::Error::into_internal_error)
    }

    fn from_value(
        _method: &str,
        value: serde_json::Value,
    ) -> Result<Self, agent_client_protocol::Error> {
        Ok(Self(agent_client_protocol::util::json_cast(&value)?))
    }
}
