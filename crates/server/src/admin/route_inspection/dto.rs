use any2api_domain::{ProtocolDialect, ProtocolOperation, ProviderEndpointId, ProviderKind};
use any2api_runtime::api::{
    RouteInspectionCandidateGroup, RouteInspectionItem, RouteInspectionOperation,
    RouteInspectionSnapshot,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct RouteInspectionResponse {
    config_revision: u64,
    items: Vec<RouteInspectionItemResponse>,
}

impl From<RouteInspectionSnapshot> for RouteInspectionResponse {
    fn from(value: RouteInspectionSnapshot) -> Self {
        Self {
            config_revision: value.config_revision().get(),
            items: value
                .items()
                .iter()
                .map(RouteInspectionItemResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct RouteInspectionItemResponse {
    public_model: String,
    ingress_protocol: ProtocolDialect,
    published: bool,
    status: &'static str,
    operations: Vec<RouteInspectionOperationResponse>,
}

impl From<&RouteInspectionItem> for RouteInspectionItemResponse {
    fn from(value: &RouteInspectionItem) -> Self {
        Self {
            public_model: value.public_model().to_owned(),
            ingress_protocol: value.ingress_protocol(),
            published: value.published(),
            status: value.status().as_str(),
            operations: value
                .operations()
                .iter()
                .map(RouteInspectionOperationResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct RouteInspectionOperationResponse {
    operation: ProtocolOperation,
    candidate_groups: Vec<RouteInspectionCandidateGroupResponse>,
}

impl From<&RouteInspectionOperation> for RouteInspectionOperationResponse {
    fn from(value: &RouteInspectionOperation) -> Self {
        Self {
            operation: value.operation(),
            candidate_groups: value
                .candidate_groups()
                .iter()
                .map(RouteInspectionCandidateGroupResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct RouteInspectionCandidateGroupResponse {
    provider_kind: ProviderKind,
    provider_endpoint_id: Option<ProviderEndpointId>,
    provider_endpoint_name: Option<String>,
    upstream_protocol_dialect: ProtocolDialect,
    enabled_candidate_count: usize,
}

impl From<&RouteInspectionCandidateGroup> for RouteInspectionCandidateGroupResponse {
    fn from(value: &RouteInspectionCandidateGroup) -> Self {
        Self {
            provider_kind: value.provider_kind(),
            provider_endpoint_id: value.provider_endpoint_id(),
            provider_endpoint_name: value.provider_endpoint_name().map(str::to_owned),
            upstream_protocol_dialect: value.upstream_protocol_dialect(),
            enabled_candidate_count: value.enabled_candidate_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolDialect, ProtocolOperation, ProviderEndpointId, ProviderKind};
    use serde_json::json;

    use super::{
        RouteInspectionCandidateGroupResponse, RouteInspectionItemResponse,
        RouteInspectionOperationResponse, RouteInspectionResponse,
    };

    #[test]
    fn response_contract_uses_only_configuration_route_state() {
        let endpoint_id = ProviderEndpointId::new();
        let response = RouteInspectionResponse {
            config_revision: 7,
            items: vec![RouteInspectionItemResponse {
                public_model: "gpt-route".to_owned(),
                ingress_protocol: ProtocolDialect::OpenAiResponses,
                published: true,
                status: "available",
                operations: vec![RouteInspectionOperationResponse {
                    operation: ProtocolOperation::Responses,
                    candidate_groups: vec![RouteInspectionCandidateGroupResponse {
                        provider_kind: ProviderKind::Codex,
                        provider_endpoint_id: Some(endpoint_id),
                        provider_endpoint_name: Some("Codex Primary".to_owned()),
                        upstream_protocol_dialect: ProtocolDialect::OpenAiChatCompletions,
                        enabled_candidate_count: 2,
                    }],
                }],
            }],
        };

        assert_eq!(
            serde_json::to_value(response).expect("serialize response"),
            json!({
                "config_revision": 7,
                "items": [{
                    "public_model": "gpt-route",
                    "ingress_protocol": "openai_responses",
                    "published": true,
                    "status": "available",
                    "operations": [{
                        "operation": "responses",
                        "candidate_groups": [{
                            "provider_kind": "codex",
                            "provider_endpoint_id": endpoint_id,
                            "provider_endpoint_name": "Codex Primary",
                            "upstream_protocol_dialect": "openai_chat_completions",
                            "enabled_candidate_count": 2
                        }]
                    }]
                }]
            })
        );
    }
}
