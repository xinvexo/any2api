mod dns;
mod origin;

#[cfg(test)]
mod tests;

pub(crate) use dns::{DnsLookupError, shared_dns_cache};
pub(crate) use origin::{OriginTarget, origin_target};
