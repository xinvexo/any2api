use any2api_server::api::{EmbeddedWebAsset, WebAssets};

include!(concat!(env!("OUT_DIR"), "/embedded_web_assets.rs"));

pub(crate) const fn assets() -> WebAssets {
    WebAssets::embedded(EMBEDDED_WEB_ASSETS)
}
