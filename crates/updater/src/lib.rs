pub mod api;

mod github;
mod github_release_updater;
mod install;
mod recovery;
mod smoke;
mod state;
mod temporary;

pub(crate) const BUILD_VERSION: &str = match option_env!("ANY2API_BUILD_VERSION") {
    Some(version) => version,
    None => "0.0.0-dev",
};

#[cfg(test)]
mod tests;
