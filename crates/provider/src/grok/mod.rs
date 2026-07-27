mod driver;
mod import;
mod oauth;
mod quota;
mod upstream_error;

pub use driver::GrokDriver;

pub fn oauth_bot_flag(token: &crate::OAuthTokenMaterial) -> Option<bool> {
    oauth::bot_flagged(token)
}

#[cfg(test)]
mod tests;
