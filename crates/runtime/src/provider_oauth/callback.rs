use subtle::ConstantTimeEq;
use url::Url;

use super::error::ProviderOAuthError;

const MAX_CALLBACK_URL_BYTES: usize = 16 * 1024;
const MAX_AUTHORIZATION_CODE_BYTES: usize = 8 * 1024;

pub(crate) struct OAuthCallback {
    pub(crate) code: String,
}

pub(crate) fn parse(
    value: &str,
    redirect_uri: &str,
    expected_state: &str,
) -> Result<OAuthCallback, ProviderOAuthError> {
    if value.is_empty() || value.len() > MAX_CALLBACK_URL_BYTES {
        return Err(ProviderOAuthError::InvalidCallback);
    }
    let callback = Url::parse(value).map_err(|_| ProviderOAuthError::InvalidCallback)?;
    let redirect = Url::parse(redirect_uri).map_err(|_| ProviderOAuthError::InvalidCallback)?;
    if !same_callback_target(&callback, &redirect) {
        return Err(ProviderOAuthError::InvalidCallback);
    }

    let mut code = None;
    let mut state = None;
    let mut denied = false;
    for (key, value) in callback.query_pairs() {
        match key.as_ref() {
            "code" if code.is_none() => code = Some(value.into_owned()),
            "state" if state.is_none() => state = Some(value.into_owned()),
            "error" => denied = true,
            _ => {}
        }
    }
    if denied {
        return Err(ProviderOAuthError::AuthorizationDenied);
    }
    let mut code = code.ok_or(ProviderOAuthError::InvalidCallback)?;
    if let Some((plain_code, embedded_state)) = code.split_once('#') {
        if let Some(query_state) = state.as_deref()
            && query_state != embedded_state
        {
            return Err(ProviderOAuthError::StateMismatch);
        }
        state = Some(embedded_state.to_owned());
        code = plain_code.to_owned();
    } else if state.is_none()
        && let Some(fragment) = callback.fragment()
        && !fragment.is_empty()
    {
        state = Some(fragment.to_owned());
    }
    if code.is_empty() || code.len() > MAX_AUTHORIZATION_CODE_BYTES {
        return Err(ProviderOAuthError::InvalidCallback);
    }
    let state = state.ok_or(ProviderOAuthError::InvalidCallback)?;
    if !constant_time_eq(state.as_bytes(), expected_state.as_bytes()) {
        return Err(ProviderOAuthError::StateMismatch);
    }
    Ok(OAuthCallback { code })
}

fn same_callback_target(callback: &Url, redirect: &Url) -> bool {
    callback.scheme() == redirect.scheme()
        && callback.username() == redirect.username()
        && callback.password() == redirect.password()
        && callback.host_str() == redirect.host_str()
        && callback.port_or_known_default() == redirect.port_or_known_default()
        && callback.path() == redirect.path()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && bool::from(left.ct_eq(right))
}
