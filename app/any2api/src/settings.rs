use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};
use ipnet::IpNet;
use secrecy::SecretString;

pub(crate) struct AppSettings {
    pub(crate) bind: SocketAddr,
    pub(crate) database_path: PathBuf,
    pub(crate) master_key_path: PathBuf,
    pub(crate) log_directory: PathBuf,
    pub(crate) web_root: PathBuf,
    pub(crate) admin_password: Option<SecretString>,
    pub(crate) trusted_proxy_cidrs: Vec<IpNet>,
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
        let master_key_path = env::var_os("ANY2API_MASTER_KEY_FILE")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| data_dir.join("master-key.json"));
        let admin_password = env::var("ANY2API_ADMIN_PASSWORD")
            .ok()
            .filter(|value| !value.is_empty())
            .map(SecretString::from);
        let trusted_proxy_cidrs = parse_trusted_proxy_cidrs()?;

        Ok(Self {
            bind,
            database_path: data_dir.join("any2api.sqlite3"),
            master_key_path,
            log_directory: data_dir.join("logs"),
            web_root,
            admin_password,
            trusted_proxy_cidrs,
        })
    }
}

fn parse_trusted_proxy_cidrs() -> Result<Vec<IpNet>> {
    let Some(value) = env::var("ANY2API_TRUSTED_PROXY_CIDRS")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(Vec::new());
    };
    value
        .split(',')
        .map(str::trim)
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("invalid trusted proxy CIDR {value}"))
        })
        .collect()
}
