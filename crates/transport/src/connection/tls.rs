use std::sync::{Arc, LazyLock};

use hyper_rustls::{FixedServerNameResolver, HttpsConnector, HttpsConnectorBuilder};
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, ServerName},
};

use crate::error::{TransportError, TransportErrorStage, TransportFailureScope};

/// Loading and parsing the platform trust store costs tens of milliseconds of
/// blocking IO, so it happens once per process and the parsed store is shared.
static NATIVE_ROOTS: LazyLock<Arc<RootCertStore>> = LazyLock::new(|| {
    let native = rustls_native_certs::load_native_certs();
    let mut roots = RootCertStore::empty();
    roots.add_parsable_certificates(native.certs);
    Arc::new(roots)
});

pub(crate) struct TlsConfigFactory {
    roots: Arc<RootCertStore>,
}

impl TlsConfigFactory {
    pub(crate) fn new(extra_roots: &[CertificateDer<'static>]) -> Result<Self, TransportError> {
        let roots = if extra_roots.is_empty() {
            Arc::clone(&NATIVE_ROOTS)
        } else {
            let mut roots = (**NATIVE_ROOTS).clone();
            for certificate in extra_roots {
                roots.add(certificate.clone()).map_err(|_| {
                    TransportError::configuration(
                        TransportErrorStage::Tls,
                        TransportFailureScope::Unattributed,
                        "configured TLS root certificate is invalid",
                    )
                })?;
            }
            Arc::new(roots)
        };
        if roots.is_empty() {
            return Err(TransportError::configuration(
                TransportErrorStage::Tls,
                TransportFailureScope::Unattributed,
                "no trusted TLS root certificates are available",
            ));
        }
        Ok(Self { roots })
    }

    /// Every cached transport client receives a fresh rustls configuration.
    /// Trust roots are immutable and shared, while the default session store
    /// belongs only to this newly constructed client.
    pub(crate) fn build(&self) -> ClientConfig {
        ClientConfig::builder()
            .with_root_certificates(Arc::clone(&self.roots))
            .with_no_client_auth()
    }
}

pub(crate) fn wrap_tls<C>(
    connector: C,
    tls_config: ClientConfig,
    server_name: ServerName<'static>,
) -> HttpsConnector<C> {
    HttpsConnectorBuilder::new()
        .with_tls_config(tls_config)
        .https_or_http()
        .with_server_name_resolver(FixedServerNameResolver::new(server_name))
        .enable_http1()
        .enable_http2()
        .wrap_connector(connector)
}
