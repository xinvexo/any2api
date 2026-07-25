use hyper_rustls::{FixedServerNameResolver, HttpsConnector, HttpsConnectorBuilder};
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, ServerName},
};

use crate::error::TransportError;

pub(crate) fn build_tls_config(
    extra_roots: &[CertificateDer<'static>],
) -> Result<ClientConfig, TransportError> {
    let native = rustls_native_certs::load_native_certs();
    let mut roots = RootCertStore::empty();
    roots.add_parsable_certificates(native.certs);
    for certificate in extra_roots {
        roots.add(certificate.clone()).map_err(|_| {
            TransportError::configuration("configured TLS root certificate is invalid")
        })?;
    }
    if roots.is_empty() {
        return Err(TransportError::configuration(
            "no trusted TLS root certificates are available",
        ));
    }
    Ok(ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
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
