//! Read-side query surface of [`RequestTelemetry`], split from the hot-path
//! recording logic in the parent module.

use std::time::{SystemTime, UNIX_EPOCH};

use any2api_domain::{
    CompletedRequestLog, HttpAccessLog, HttpAccessLogSummary, LogPage, LogPageCursor, RequestId,
    RequestLog, RequestLogFilter,
};
use any2api_storage::api::{
    GatewayApiKeyUsageSummary, RequestLogOverview, RequestLogOverviewRange, StorageError,
    UpstreamCredentialUsageSummary,
};

use super::RequestTelemetry;

impl RequestTelemetry {
    pub async fn list(
        &self,
        since_ms: u64,
        filter: &RequestLogFilter,
        cursor: Option<LogPageCursor>,
        limit: u32,
    ) -> Result<LogPage<RequestLog>, StorageError> {
        match &self.request_logs {
            Some(repository) => {
                repository
                    .list_request_logs(since_ms, filter, cursor, limit)
                    .await
            }
            None => Ok(LogPage::empty()),
        }
    }

    pub async fn get(
        &self,
        request_id: RequestId,
    ) -> Result<Option<CompletedRequestLog>, StorageError> {
        match &self.request_logs {
            Some(repository) => repository.get_request_log(request_id).await,
            None => Ok(None),
        }
    }

    pub async fn list_http_access_logs(
        &self,
        since_ms: u64,
        cursor: Option<LogPageCursor>,
        limit: u32,
    ) -> Result<LogPage<HttpAccessLogSummary>, StorageError> {
        match &self.http_access_logs {
            Some(repository) => {
                repository
                    .list_http_access_logs(since_ms, cursor, limit)
                    .await
            }
            None => Ok(LogPage::empty()),
        }
    }

    pub async fn get_http_access_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<HttpAccessLog>, StorageError> {
        match &self.http_access_logs {
            Some(repository) => repository.get_http_access_log(request_id).await,
            None => Ok(None),
        }
    }

    pub async fn gateway_key_usage(&self) -> Result<Vec<GatewayApiKeyUsageSummary>, StorageError> {
        match &self.gateway_usage_repository {
            Some(repository) => repository.list_gateway_api_key_usage().await,
            None => Ok(Vec::new()),
        }
    }

    pub async fn upstream_credential_usage(
        &self,
    ) -> Result<Vec<UpstreamCredentialUsageSummary>, StorageError> {
        match &self.upstream_usage_repository {
            Some(repository) => repository.list_upstream_credential_usage().await,
            None => Ok(Vec::new()),
        }
    }

    pub async fn overview_usage(
        &self,
        range: RequestLogOverviewRange,
    ) -> Result<RequestLogOverview, StorageError> {
        match &self.request_logs {
            Some(repository) => repository.request_log_overview(range).await,
            None => Ok(RequestLogOverview::empty(range, unix_now_ms()?)),
        }
    }
}

fn unix_now_ms() -> Result<u64, StorageError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .map_err(|_| StorageError::CorruptTelemetry)
}
