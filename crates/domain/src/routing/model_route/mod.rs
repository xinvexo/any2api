mod configuration;
mod route;

#[cfg(test)]
mod configuration_tests;

pub use configuration::ModelRouteConfiguration;
pub use route::{ModelRoute, ModelRouteDraft, ModelRouteValidationError};
