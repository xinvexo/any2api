mod circuit;
mod policy;
mod registry;
mod runtime;

pub(crate) use policy::ReliabilityPolicy;
pub(crate) use registry::{HealthBindings, HealthRegistry};
pub(crate) use runtime::{
    AttemptHealth, CredentialHealthRuntime, EndpointHealthRuntime, HealthAcquireError,
    ProxyHealthRuntime,
};

#[cfg(test)]
mod tests;
