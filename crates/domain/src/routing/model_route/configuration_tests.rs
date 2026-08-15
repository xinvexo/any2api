use crate::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, FallbackTier, ModelRoute,
    ModelRouteConfiguration, ModelRouteDraft, ModelRouteId, ModelRouteValidationError,
    ProtocolDialect, ProviderCredential, ProviderCredentialConfiguration, ProviderCredentialDraft,
    ProviderCredentialModel, ProviderEndpoint, ProviderEndpointConfiguration,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyConfiguration, ProxyProfileId,
    RouteTargetDraft, RouteTargetId,
};

#[test]
fn aliased_credential_models_join_the_public_route_with_their_upstream_names() {
    let renamed_id = ProviderEndpointId::new();
    let standard_id = ProviderEndpointId::new();
    let endpoints = ProviderEndpointConfiguration::new(vec![
        named_endpoint(renamed_id, "ganen"),
        named_endpoint(standard_id, "standard"),
    ])
    .expect("endpoint configuration");
    let credentials = credentials(
        &endpoints,
        vec![
            credential(
                renamed_id,
                "Renamed",
                vec![model("gpt-5.6-sol-ganen", Some("gpt-5.6-sol"))],
            ),
            credential(standard_id, "Standard", vec![model("gpt-5.6-sol", None)]),
        ],
    );

    let routes = ModelRouteConfiguration::from_credentials(&credentials, &endpoints)
        .expect("derived routes");

    assert_eq!(routes.routes().len(), 1);
    let route = &routes.routes()[0];
    assert_eq!(route.public_model().as_str(), "gpt-5.6-sol");
    let mut targets = route
        .targets()
        .iter()
        .map(|target| {
            (
                target.provider_endpoint_id(),
                target.upstream_model().as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    targets.sort();
    let mut expected = vec![
        (renamed_id, "gpt-5.6-sol-ganen".to_owned()),
        (standard_id, "gpt-5.6-sol".to_owned()),
    ];
    expected.sort();
    assert_eq!(targets, expected);
    let ids = route
        .targets()
        .iter()
        .map(|target| target.id())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 2);
}

#[test]
fn endpoint_model_mappings_must_agree_across_credentials_in_both_directions() {
    let endpoint_id = ProviderEndpointId::new();
    let endpoints = ProviderEndpointConfiguration::new(vec![named_endpoint(endpoint_id, "ganen")])
        .expect("endpoint configuration");

    let forward = credentials(
        &endpoints,
        vec![
            credential(
                endpoint_id,
                "Aliased",
                vec![model("gpt-5.6-sol-ganen", Some("gpt-5.6-sol"))],
            ),
            credential(endpoint_id, "Plain", vec![model("gpt-5.6-sol", None)]),
        ],
    );
    assert!(matches!(
        ModelRouteConfiguration::from_credentials(&forward, &endpoints)
            .expect_err("conflicting upstream models"),
        ModelRouteValidationError::ConflictingUpstreamModel { endpoint, .. }
            if endpoint == "ganen"
    ));

    let reverse = credentials(
        &endpoints,
        vec![
            credential(
                endpoint_id,
                "Aliased",
                vec![model("gpt-5.6-sol-ganen", Some("gpt-5.6-sol"))],
            ),
            credential(endpoint_id, "Plain", vec![model("gpt-5.6-sol-ganen", None)]),
        ],
    );
    assert!(matches!(
        ModelRouteConfiguration::from_credentials(&reverse, &endpoints)
            .expect_err("conflicting public models"),
        ModelRouteValidationError::ConflictingPublicModel { endpoint, .. }
            if endpoint == "ganen"
    ));

    let agreeing = credentials(
        &endpoints,
        vec![
            credential(
                endpoint_id,
                "First",
                vec![model("gpt-5.6-sol-ganen", Some("gpt-5.6-sol"))],
            ),
            credential(
                endpoint_id,
                "Second",
                vec![model("gpt-5.6-sol-ganen", Some("gpt-5.6-sol"))],
            ),
        ],
    );
    let routes = ModelRouteConfiguration::from_credentials(&agreeing, &endpoints)
        .expect("agreeing credentials");
    assert_eq!(routes.routes().len(), 1);
    assert_eq!(routes.routes()[0].targets().len(), 1);
}

#[test]
fn public_models_are_unique_per_protocol_and_targets_must_match_endpoint_dialect() {
    let codex_id = ProviderEndpointId::new();
    let claude_id = ProviderEndpointId::new();
    let endpoints = ProviderEndpointConfiguration::new(vec![
        endpoint(
            codex_id,
            ProviderKind::Codex,
            ProtocolDialect::OpenAiResponses,
        ),
        endpoint(
            claude_id,
            ProviderKind::Claude,
            ProtocolDialect::AnthropicMessages,
        ),
    ])
    .expect("endpoint configuration");
    let responses = route("shared", ProtocolDialect::OpenAiResponses, codex_id);
    let messages = route("shared", ProtocolDialect::AnthropicMessages, claude_id);
    assert!(ModelRouteConfiguration::new(vec![responses.clone(), messages], &endpoints).is_ok());

    assert_eq!(
        ModelRouteConfiguration::new(
            vec![
                responses,
                route("shared", ProtocolDialect::OpenAiResponses, codex_id),
            ],
            &endpoints,
        )
        .expect_err("duplicate public model"),
        ModelRouteValidationError::DuplicatePublicModel
    );
    assert!(matches!(
        ModelRouteConfiguration::new(
            vec![route("wrong", ProtocolDialect::OpenAiResponses, claude_id)],
            &endpoints,
        ),
        Err(ModelRouteValidationError::IncompatibleTargetProtocol(id)) if id == claude_id
    ));
}

fn endpoint(
    id: ProviderEndpointId,
    kind: ProviderKind,
    dialect: ProtocolDialect,
) -> ProviderEndpoint {
    ProviderEndpoint::create(
        id,
        ProviderEndpointDraft::new(
            format!("{kind:?}"),
            kind,
            "https://api.example.com",
            dialect,
            None,
            true,
        )
        .expect("endpoint draft"),
    )
    .expect("endpoint")
}

fn route(
    public_model: &str,
    dialect: ProtocolDialect,
    endpoint_id: ProviderEndpointId,
) -> ModelRoute {
    ModelRoute::create(
        ModelRouteId::new(),
        ModelRouteDraft::new(
            public_model,
            dialect,
            None,
            true,
            vec![
                RouteTargetDraft::new(
                    RouteTargetId::new(),
                    endpoint_id,
                    "upstream",
                    dialect,
                    FallbackTier::default(),
                    true,
                )
                .expect("target draft"),
            ],
        )
        .expect("route draft"),
    )
}

fn named_endpoint(id: ProviderEndpointId, name: &str) -> ProviderEndpoint {
    ProviderEndpoint::create(
        id,
        ProviderEndpointDraft::new(
            name,
            ProviderKind::Codex,
            "https://api.example.com",
            ProtocolDialect::OpenAiResponses,
            None,
            true,
        )
        .expect("endpoint draft"),
    )
    .expect("endpoint")
}

fn model(upstream: &str, public: Option<&str>) -> ProviderCredentialModel {
    ProviderCredentialModel::new(upstream, public.map(str::to_owned)).expect("credential model")
}

fn credential(
    endpoint_id: ProviderEndpointId,
    label: &str,
    models: Vec<ProviderCredentialModel>,
) -> ProviderCredential {
    let draft = ProviderCredentialDraft::new(
        label,
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        None,
        true,
    )
    .expect("credential draft");
    let fingerprint =
        CredentialSecretFingerprint::new([0x5a; 32], Some("test".to_owned())).expect("fingerprint");
    ProviderCredential::restore(
        CredentialId::new(),
        endpoint_id,
        draft,
        fingerprint,
        1,
        1,
        1,
        models,
    )
    .expect("credential")
}

fn credentials(
    endpoints: &ProviderEndpointConfiguration,
    credentials: Vec<ProviderCredential>,
) -> ProviderCredentialConfiguration {
    ProviderCredentialConfiguration::new(credentials, endpoints, &ProxyConfiguration::initial())
        .expect("credential configuration")
}
