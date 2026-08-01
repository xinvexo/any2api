use crate::{proxy::ProxyAuthMaterialError, routing::RoutingCredentialCompileError};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{detail}")]
pub struct SnapshotCompileError {
    detail: SnapshotCompileErrorDetail,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
enum SnapshotCompileErrorDetail {
    #[error("proxy authentication material is inconsistent: {0}")]
    ProxyAuthentication(ProxyAuthMaterialError),
    #[error("routing credential material is inconsistent: {0}")]
    RoutingCredential(RoutingCredentialCompileError),
}

impl From<ProxyAuthMaterialError> for SnapshotCompileError {
    fn from(error: ProxyAuthMaterialError) -> Self {
        Self {
            detail: SnapshotCompileErrorDetail::ProxyAuthentication(error),
        }
    }
}

impl From<RoutingCredentialCompileError> for SnapshotCompileError {
    fn from(error: RoutingCredentialCompileError) -> Self {
        Self {
            detail: SnapshotCompileErrorDetail::RoutingCredential(error),
        }
    }
}
