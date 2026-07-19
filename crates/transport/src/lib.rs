pub mod api;

mod client_cache;
mod error;
mod origin_resolution;
mod proxy_url;
mod reqwest_manager;

pub use error::{
    TransportConfigurationError, TransportError, TransportErrorStage, TransportFailureScope,
};
pub use reqwest_manager::ReqwestTransportManager;

#[cfg(test)]
mod http_connect_tests;
#[cfg(test)]
mod reqwest_manager_tests;
#[cfg(test)]
mod transport_failure_scope_tests;
