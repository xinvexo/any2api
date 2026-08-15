mod driver;
mod headers;
mod identity;
mod import;
mod model_catalog;
mod oauth;
mod quota;
mod upstream_error;

pub use driver::GrokDriver;

pub fn oauth_bot_flag(token: &crate::OAuthTokenMaterial) -> Option<bool> {
    oauth::bot_flagged(token)
}

#[cfg(test)]
mod tests;
