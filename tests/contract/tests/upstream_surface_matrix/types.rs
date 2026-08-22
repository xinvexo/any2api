use std::collections::BTreeSet;

use any2api_domain::ProviderKind;
use any2api_transport::api::TransportRequest;

pub(super) struct SurfaceCase {
    pub(super) name: String,
    pub(super) provider: ProviderKind,
    pub(super) surface: Surface,
    pub(super) auth_class: &'static str,
    pub(super) target: String,
    pub(super) request: TransportRequest,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Surface {
    DataDirect,
    DataBridge,
    OAuthToken,
    OAuthQuota,
}

impl Surface {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::DataDirect => "data_direct",
            Self::DataBridge => "data_bridge",
            Self::OAuthToken => "oauth_token",
            Self::OAuthQuota => "oauth_quota",
        }
    }
}

pub(super) fn assert_complete_matrix(cases: &[SurfaceCase]) {
    let names = cases
        .iter()
        .map(|case| case.name.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), cases.len(), "surface names must be unique");
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.surface == Surface::DataDirect)
            .count(),
        23,
        "17 API Key and 6 OAuth direct operations"
    );
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.surface == Surface::DataBridge)
            .count(),
        8,
        "two registered bridges across four Chat-capable Providers"
    );
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.surface == Surface::OAuthToken)
            .count(),
        7
    );
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.surface == Surface::OAuthQuota)
            .count(),
        6
    );
    assert_eq!(cases.len(), 44);
}
