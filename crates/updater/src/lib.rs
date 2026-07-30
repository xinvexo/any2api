pub mod api;

mod github;
mod install;
mod service;
mod state;

pub(crate) const BUILD_VERSION: &str = match option_env!("ANY2API_BUILD_VERSION") {
    Some(version) => version,
    None => "0.0.0-dev",
};

#[cfg(test)]
mod tests;
