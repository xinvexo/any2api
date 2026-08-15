use serde::Deserialize;

use crate::log_pagination::{LogPageRequest, validate_system_log_page};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SystemLogListQuery {
    cursor: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    #[serde(default = "show_admin_operations_by_default")]
    show_admin_operations: bool,
}

pub(super) struct ValidatedSystemLogListQuery {
    pub(super) page: LogPageRequest,
    pub(super) show_admin_operations: bool,
}

impl SystemLogListQuery {
    pub(super) fn validate(self) -> Option<ValidatedSystemLogListQuery> {
        let page = validate_system_log_page(
            self.cursor,
            self.page,
            self.page_size,
            self.show_admin_operations,
        )?;
        Some(ValidatedSystemLogListQuery {
            page,
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
