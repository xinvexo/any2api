mod body_timeout;
mod cache;
mod construction;
mod deadline;
mod dns;
mod failure;
mod pinned;
mod request_body;
mod reqwest;

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod fingerprint_tests;
#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod timeout_tests;

pub use reqwest::ReqwestTransportManager;
