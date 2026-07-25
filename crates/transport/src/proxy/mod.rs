mod credentials;
pub(crate) mod tcp;
pub(crate) mod url;

#[cfg(test)]
mod failure_scope_tests;
#[cfg(test)]
mod tests;

pub use credentials::ProxyCredentials;
