use any2api_domain::HttpAccessLogFilter;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;

use crate::log_cursor::{LogBatchRequest, validate_system_log_batch};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SystemLogListQuery {
    cursor: Option<String>,
    #[serde(default = "show_admin_operations_by_default")]
    show_admin_operations: bool,
    status_code: Option<u16>,
    client_ip: Option<std::net::IpAddr>,
    path: Option<String>,
}

pub(super) struct ValidatedSystemLogListQuery {
    pub(super) batch: LogBatchRequest,
    pub(super) filter: HttpAccessLogFilter,
    pub(super) scope: String,
}

impl SystemLogListQuery {
    pub(super) fn validate(self) -> Option<ValidatedSystemLogListQuery> {
        if self
            .status_code
            .is_some_and(|status| !(100..=599).contains(&status))
            || self.path.as_ref().is_some_and(|path| path.len() > 256)
        {
            return None;
        }
        let filter = HttpAccessLogFilter {
            show_admin_operations: self.show_admin_operations,
            status_code: self.status_code,
            client_ip: self.client_ip.map(|address| address.to_canonical()),
            path: self.path.filter(|path| !path.is_empty()),
        };
        let scope = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&(
                filter.show_admin_operations,
                filter.status_code,
                filter.client_ip,
                &filter.path,
            ))
            .ok()?,
        );
        let batch = validate_system_log_batch(self.cursor, &scope)?;
        Some(ValidatedSystemLogListQuery {
            batch,
            filter,
            scope,
        })
    }
}

const fn show_admin_operations_by_default() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_showing_admin_operations_and_accepts_explicit_hiding() {
        let default = serde_json::from_value::<SystemLogListQuery>(serde_json::json!({}))
            .expect("default query")
            .validate()
            .expect("valid default query");
        assert!(default.filter.show_admin_operations);

        let hidden = serde_json::from_value::<SystemLogListQuery>(serde_json::json!({
            "show_admin_operations": false
        }))
        .expect("explicit filter")
        .validate()
        .expect("valid filtered query");
        assert!(!hidden.filter.show_admin_operations);
    }

    #[test]
    fn rejects_unknown_filter_fields() {
        assert!(
            serde_json::from_value::<SystemLogListQuery>(serde_json::json!({
                "include_admin_logs": false
            }))
            .is_err()
        );
    }
}
