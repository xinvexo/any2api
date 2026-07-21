pub mod api;

mod affinity;
mod auxiliary_scheduler;
mod config_command;
mod config_publish_error;
mod credential_auth;
mod credential_runtime;
mod gateway_api_key_publisher;
mod gateway_api_key_token;
mod health;
mod provider_api_key_secret;
mod proxy_auth;
mod proxy_password_secret;
mod proxy_test;
mod public_request;
mod publish_task;
mod published_snapshot;
mod publisher;
mod queue;
mod registry;
mod request_telemetry;
mod route_candidates;
mod route_tier_cursor;
mod scheduler;
mod scheduler_epoch;

#[cfg(test)]
mod auxiliary_scheduler_tests;

#[cfg(test)]
mod credential_runtime_tests;
#[cfg(test)]
mod gateway_api_key_publisher_tests;
#[cfg(test)]
mod model_route_publisher_tests;
#[cfg(test)]
mod published_snapshot_tests;
#[cfg(test)]
mod publisher_tests;
#[cfg(test)]
mod request_telemetry_tests;
