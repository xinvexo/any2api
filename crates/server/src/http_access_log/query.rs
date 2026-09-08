use serde::Deserialize;

use crate::log_cursor::{LogBatchRequest, validate_system_log_batch};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SystemLogListQuery {
    cursor: Option<String>,
    #[serde(default = "show_admin_operations_by_default")]
    show_admin_operations: bool,
}

pub(super) struct ValidatedSystemLogListQuery {
    pub(super) batch: LogBatchRequest,
    pub(super) show_admin_operations: bool,
}

impl SystemLogListQuery {
    pub(super) fn validate(self) -> Option<ValidatedSystemLogListQuery> {
        let batch = validate_system_log_batch(self.cursor, self.show_admin_operations)?;
        Some(ValidatedSystemLogListQuery {
            batch,
            show_admin_operations: self.show_admin_operations,
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
        assert!(default.show_admin_operations);

        let hidden = serde_json::from_value::<SystemLogListQuery>(serde_json::json!({
            "show_admin_operations": false
        }))
        .expect("explicit filter")
        .validate()
        .expect("valid filtered query");
        assert!(!hidden.show_admin_operations);
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
