use any2api_contract_tests::build_public_request_components;
use any2api_domain::{ProtocolOperation, ProviderKind, ProxyProfile};
use any2api_transport::api::ReqwestTransportManager;

#[path = "upstream_surface_matrix/control.rs"]
mod control;
#[path = "upstream_surface_matrix/data.rs"]
mod data;
#[path = "upstream_surface_matrix/raw.rs"]
mod raw;
#[path = "upstream_surface_matrix/types.rs"]
mod types;

use control::{append_control_plane_cases, oauth_tokens};
use data::{DataCaseSpec, DataCredential, api_key_base_url, data_case};
use raw::capture;
use types::{Surface, assert_complete_matrix};

const FIXTURE: &str = include_str!("../testdata/upstream-surface-matrix.txt");

#[tokio::test]
async fn registered_upstream_surfaces_match_the_raw_http1_matrix() {
    let components = build_public_request_components().expect("public request components");
    let protocols = components.protocol_registry();
    let providers = components.provider_registry();
    let tokens = oauth_tokens();
    let mut cases = Vec::new();

    for kind in ProviderKind::ALL {
        let driver = providers.get(kind).expect("registered Provider driver");
        let base_url = api_key_base_url(kind);
        for operation in ProtocolOperation::ALL {
            if driver.descriptor().supports_api_key_operation(operation)
                && driver.endpoint_plan(&base_url, operation).is_ok()
            {
                cases.push(
                    data_case(
                        protocols,
                        driver.as_ref(),
                        DataCaseSpec {
                            ingress: operation.dialect(),
                            upstream: operation.dialect(),
                            operation,
                            base_url: &base_url,
                            credential: DataCredential::ApiKey,
                            surface: Surface::DataDirect,
                        },
                    )
                    .await,
                );
            }
        }
    }

    for (kind, token) in &tokens {
        let driver = providers.get(*kind).expect("OAuth Provider driver");
        let routing = driver.oauth_routing().expect("OAuth routing facet");
        let profile = routing
            .oauth_routing_profile(token)
            .expect("OAuth routing profile");
        for operation in ProtocolOperation::ALL {
            if driver.descriptor().supports_oauth_operation(operation) {
                assert_eq!(operation.dialect(), profile.protocol_dialect());
                cases.push(
                    data_case(
                        protocols,
                        driver.as_ref(),
                        DataCaseSpec {
                            ingress: operation.dialect(),
                            upstream: profile.protocol_dialect(),
                            operation,
                            base_url: profile.base_url(),
                            credential: DataCredential::OAuth(token),
                            surface: Surface::DataDirect,
                        },
                    )
                    .await,
                );
            }
        }
    }

    let mut bridges = protocols
        .iter_bridges()
        .map(|(pair, _)| *pair)
        .collect::<Vec<_>>();
    bridges.sort_unstable();
    for (ingress, upstream) in bridges {
        let capabilities = protocols
            .pair_capabilities(ingress, upstream)
            .expect("registered bridge capabilities");
        for operation in capabilities.operations {
            for kind in ProviderKind::ALL {
                let driver = providers.get(kind).expect("registered Provider driver");
                if !driver.descriptor().supports_protocol(upstream) {
                    continue;
                }
                let base_url = api_key_base_url(kind);
                cases.push(
                    data_case(
                        protocols,
                        driver.as_ref(),
                        DataCaseSpec {
                            ingress,
                            upstream,
                            operation,
                            base_url: &base_url,
                            credential: DataCredential::ApiKey,
                            surface: Surface::DataBridge,
                        },
                    )
                    .await,
                );
            }
        }
    }

    append_control_plane_cases(&mut cases, providers, &tokens);
    assert_complete_matrix(&cases);
    cases.sort_unstable_by(|left, right| left.name.cmp(&right.name));

    let manager = ReqwestTransportManager::default();
    let direct = ProxyProfile::direct();
    let mut captures = Vec::with_capacity(cases.len());
    for case in cases {
        captures.push(capture(&manager, &direct, case).await);
    }
    let actual = format!("{}\n", captures.join("\n\n"));
    assert_eq!(actual, FIXTURE, "upstream surface fixture changed");
}
