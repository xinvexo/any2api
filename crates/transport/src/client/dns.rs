use std::net::SocketAddr;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use crate::resolution::shared_dns_cache;

/// reqwest resolver backed by the shared process-level DNS cache; the client
/// connects to exactly the cached addresses, which keeps strict-SSRF address
/// pinning while decoupling client identity from DNS results.
pub(super) struct CachedDnsResolver;

impl Resolve for CachedDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let addresses = shared_dns_cache().resolve(name.as_str()).await?;
            let addresses: Addrs = Box::new(
                addresses
                    .iter()
                    .map(|address| SocketAddr::new(*address, 0))
                    .collect::<Vec<_>>()
                    .into_iter(),
            );
            Ok(addresses)
        })
    }
}
