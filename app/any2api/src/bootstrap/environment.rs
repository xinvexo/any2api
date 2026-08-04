use std::{env, net::SocketAddr, num::NonZeroUsize, path::PathBuf};

use anyhow::{Context, Result};
use secrecy::SecretString;

use super::listener;

pub(super) struct StartupSettings {
    pub(crate) bind: SocketAddr,
    pub(crate) max_connections: NonZeroUsize,
    pub(crate) database_path: PathBuf,
    pub(crate) log_directory: PathBuf,
    pub(crate) web_root: Option<PathBuf>,
    pub(crate) admin_password: Option<SecretString>,
}

impl StartupSettings {
    pub(super) fn from_env() -> Result<Self> {
        let bind = env::var("ANY2API_BIND")
            .unwrap_or_else(|_| "127.0.0.1:3210".to_owned())
            .parse()
            .context("ANY2API_BIND must be a valid socket address")?;
        let max_connections = env::var("ANY2API_MAX_CONNECTIONS")
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| {
                value
                    .parse::<NonZeroUsize>()
                    .context("ANY2API_MAX_CONNECTIONS must be a positive integer")
            })
            .transpose()?
            .unwrap_or(listener::DEFAULT_MAX_CONNECTIONS);
        let data_dir = env::var_os("ANY2API_DATA_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("data"));
        let web_root = env::var_os("ANY2API_WEB_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let admin_password = env::var("ANY2API_ADMIN_PASSWORD")
            .ok()
            .filter(|value| !value.is_empty())
            .map(SecretString::from);
        Ok(Self {
            bind,
            max_connections,
            database_path: data_dir.join("any2api.sqlite3"),
            log_directory: data_dir.join("logs"),
            web_root,
            admin_password,
        })
    }
}
