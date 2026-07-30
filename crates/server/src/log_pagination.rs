use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

const DEFAULT_PAGE_SIZE: u32 = 20;
const MAX_PAGE_SIZE: u32 = 100;
const LOG_WINDOW_MS: u64 = 3 * 24 * 60 * 60 * 1_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LogListQuery {
    page: Option<u32>,
    page_size: Option<u32>,
}

#[derive(Clone, Copy)]
pub(crate) struct LogPageRequest {
    pub(crate) page: u32,
    pub(crate) page_size: u32,
    pub(crate) offset: u64,
    pub(crate) since_ms: u64,
}

impl LogListQuery {
    pub(crate) fn validate(self) -> Option<LogPageRequest> {
        let page = self.page.unwrap_or(1);
        let page_size = self.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        if page == 0 || !(1..=MAX_PAGE_SIZE).contains(&page_size) {
            return None;
        }
        Some(LogPageRequest {
            page,
            page_size,
            offset: u64::from(page - 1) * u64::from(page_size),
            since_ms: unix_time_ms().saturating_sub(LOG_WINDOW_MS),
        })
    }
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_validates_log_pages() {
        let page = LogListQuery {
            page: None,
            page_size: None,
        }
        .validate()
        .expect("default page");
        assert_eq!((page.page, page.page_size, page.offset), (1, 20, 0));

        let page = LogListQuery {
            page: Some(3),
            page_size: Some(50),
        }
        .validate()
        .expect("third page");
        assert_eq!((page.page, page.page_size, page.offset), (3, 50, 100));
        assert!(
            LogListQuery {
                page: Some(0),
                page_size: Some(20),
            }
            .validate()
            .is_none()
        );
    }
}
