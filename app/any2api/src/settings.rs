use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};

pub(crate) struct AppSettings {
    pub(crate) bind: SocketAddr,
    pub(crate) database_path: PathBuf,
    pub(crate) web_root: PathBuf,
}

impl AppSettings {
    pub(crate) fn from_env() -> Result<Self> {
        let bind = env::var("ANY2API_BIND")
            .unwrap_or_else(|_| "127.0.0.1:3210".to_owned())
            .parse()
            .context("ANY2API_BIND must be a valid socket address")?;
        let data_dir = env::var_os("ANY2API_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("data"));
        let web_root = env::var_os("ANY2API_WEB_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("web/dist"));

        Ok(Self {
            bind,
            database_path: data_dir.join("any2api.sqlite3"),
            web_root,
        })
    }
}
