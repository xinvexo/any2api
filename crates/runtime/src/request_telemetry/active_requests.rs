use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use any2api_domain::{
    ActiveRequestLog, CredentialId, OAuthAccountId, ProviderEndpointId, ProxyProfileId, RequestId,
    RequestLogFilter,
};

use super::{changes::LogChangeNotifier, telemetry::RequestTelemetry};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveRequestLogPage {
    pub items: Vec<ActiveRequestLog>,
    pub total: u64,
}

impl ActiveRequestLogPage {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            items: Vec::new(),
            total: 0,
        }
    }
}

#[derive(Clone)]
pub(super) struct ActiveRequestRegistry {
    entries: Arc<Mutex<BTreeMap<RequestId, ActiveRequestLog>>>,
    changes: LogChangeNotifier,
}

impl ActiveRequestRegistry {
    pub(super) fn new(changes: LogChangeNotifier) -> Self {
        Self {
            entries: Arc::new(Mutex::new(BTreeMap::new())),
            changes,
        }
    }

    pub(super) fn register(&self, log: ActiveRequestLog) {
        self.entries
            .lock()
            .expect("active request registry")
            .insert(log.request_id, log);
        self.changes.active_requests_changed();
    }

    pub(super) fn update_metadata(
        &self,
        request_id: RequestId,
        public_model: String,
        thinking_level: Option<String>,
        is_stream: bool,
    ) {
        let changed = {
            let mut entries = self.entries.lock().expect("active request registry");
            let Some(log) = entries.get_mut(&request_id) else {
                return;
            };
            let changed = log.public_model.as_ref() != Some(&public_model)
                || log.thinking_level != thinking_level
                || log.is_stream != Some(is_stream);
            log.public_model = Some(public_model);
            log.thinking_level = thinking_level;
            log.is_stream = Some(is_stream);
            changed
        };
        if changed {
            self.changes.active_requests_changed();
        }
    }

    pub(super) fn begin_attempt(
        &self,
        request_id: RequestId,
        attempt_count: u32,
        provider_endpoint_id: Option<ProviderEndpointId>,
        credential_id: Option<CredentialId>,
        oauth_account_id: Option<OAuthAccountId>,
        proxy_profile_id: ProxyProfileId,
    ) {
        let changed = {
            let mut entries = self.entries.lock().expect("active request registry");
            let Some(log) = entries.get_mut(&request_id) else {
                return;
            };
            let changed = log.attempt_count != attempt_count
                || log.provider_endpoint_id != provider_endpoint_id
                || log.credential_id != credential_id
                || log.oauth_account_id != oauth_account_id
                || log.proxy_profile_id != Some(proxy_profile_id);
            log.attempt_count = attempt_count;
            log.provider_endpoint_id = provider_endpoint_id;
            log.credential_id = credential_id;
            log.oauth_account_id = oauth_account_id;
            log.proxy_profile_id = Some(proxy_profile_id);
            changed
        };
        if changed {
            self.changes.active_requests_changed();
        }
    }

    pub(super) fn list(&self, filter: &RequestLogFilter, limit: u32) -> ActiveRequestLogPage {
        if filter.outcome().is_some() || limit == 0 {
            return ActiveRequestLogPage::empty();
        }
        let entries = self.entries.lock().expect("active request registry");
        let mut items = entries
            .values()
            .filter(|log| matches_filter(log, filter))
            .cloned()
            .collect::<Vec<_>>();
        drop(entries);
        items.sort_by(|left, right| {
            right
                .started_at_ms
                .cmp(&left.started_at_ms)
                .then_with(|| right.request_id.cmp(&left.request_id))
        });
        let total = u64::try_from(items.len()).expect("active request count fits u64");
        items.truncate(limit as usize);
        ActiveRequestLogPage { items, total }
    }

    pub(super) fn remove(&self, request_id: RequestId, notify: bool) {
        let removed = self
            .entries
            .lock()
            .expect("active request registry")
            .remove(&request_id)
            .is_some();
        if removed && notify {
            self.changes.active_requests_changed();
        }
    }

    pub(super) fn remove_many<'a>(
        &self,
        request_ids: impl IntoIterator<Item = &'a RequestId>,
        notify: bool,
    ) {
        let removed = {
            let mut entries = self.entries.lock().expect("active request registry");
            request_ids
                .into_iter()
                .fold(false, |removed, id| entries.remove(id).is_some() || removed)
        };
        if removed && notify {
            self.changes.active_requests_changed();
        }
    }

    pub(super) fn clear(&self) {
        self.entries
            .lock()
            .expect("active request registry")
            .clear();
    }
}

fn matches_filter(log: &ActiveRequestLog, filter: &RequestLogFilter) -> bool {
    filter
        .public_model()
        .is_none_or(|model| log.public_model.as_deref() == Some(model.as_str()))
        && filter
            .gateway_api_key_id()
            .is_none_or(|id| log.gateway_api_key_id == id)
}

impl RequestTelemetry {
    pub(crate) fn register_active_request(&self, log: ActiveRequestLog) {
        self.active_requests.register(log);
    }

    pub(crate) fn update_active_request_metadata(
        &self,
        request_id: RequestId,
        public_model: String,
        thinking_level: Option<String>,
        is_stream: bool,
    ) {
        self.active_requests
            .update_metadata(request_id, public_model, thinking_level, is_stream);
    }

    pub(crate) fn update_active_request_attempt(
        &self,
        request_id: RequestId,
        attempt_count: u32,
        provider_endpoint_id: Option<ProviderEndpointId>,
        credential_id: Option<CredentialId>,
        oauth_account_id: Option<OAuthAccountId>,
        proxy_profile_id: ProxyProfileId,
    ) {
        self.active_requests.begin_attempt(
            request_id,
            attempt_count,
            provider_endpoint_id,
            credential_id,
            oauth_account_id,
            proxy_profile_id,
        );
    }

    #[must_use]
    pub fn list_active_requests(
        &self,
        filter: &RequestLogFilter,
        limit: u32,
    ) -> ActiveRequestLogPage {
        self.active_requests.list(filter, limit)
    }

    #[must_use]
    pub fn subscribe_active_request_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.subscribe_active_requests()
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use any2api_domain::{
        ActiveRequestLog, ConfigRevision, CredentialId, GatewayApiKeyId, ProtocolDialect,
        ProtocolOperation, ProviderEndpointId, ProxyProfileId, PublicModelName, RequestId,
        RequestLogFilter, RequestLogOutcomeFilter,
    };

    use super::{ActiveRequestRegistry, LogChangeNotifier};

    #[test]
    fn active_requests_are_updated_filtered_and_sorted() {
        let registry = ActiveRequestRegistry::new(LogChangeNotifier::new());
        let first = active_log(10, "gpt-a");
        let second = active_log(20, "gpt-b");
        registry.register(first.clone());
        registry.register(second.clone());

        let page = registry.list(&RequestLogFilter::default(), 10);
        assert_eq!(page.total, 2);
        assert_eq!(page.items[0].request_id, second.request_id);
        assert_eq!(
            registry.list(&RequestLogFilter::default(), 1).items.len(),
            1
        );

        registry.update_metadata(first.request_id, "gpt-c".into(), Some("high".into()), true);
        let endpoint_id = ProviderEndpointId::new();
        let credential_id = CredentialId::new();
        let proxy_profile_id = ProxyProfileId::new();
        registry.begin_attempt(
            first.request_id,
            1,
            Some(endpoint_id),
            Some(credential_id),
            None,
            proxy_profile_id,
        );
        let filter = RequestLogFilter::new(
            None,
            Some(PublicModelName::new("gpt-c").expect("public model")),
            None,
        );
        let page = registry.list(&filter, 10);
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].thinking_level.as_deref(), Some("high"));
        assert_eq!(page.items[0].is_stream, Some(true));
        assert_eq!(page.items[0].provider_endpoint_id, Some(endpoint_id));
        assert_eq!(page.items[0].credential_id, Some(credential_id));
        assert_eq!(page.items[0].proxy_profile_id, Some(proxy_profile_id));
        assert_eq!(page.items[0].attempt_count, 1);

        let final_outcome_filter =
            RequestLogFilter::new(Some(RequestLogOutcomeFilter::Success), None, None);
        assert_eq!(registry.list(&final_outcome_filter, 10).total, 0);
    }

    fn active_log(started_at_ms: u64, model: &str) -> ActiveRequestLog {
        ActiveRequestLog {
            request_id: RequestId::new(),
            started_at_ms,
            client_ip: "203.0.113.8".parse::<IpAddr>().expect("client IP"),
            config_revision: ConfigRevision::INITIAL,
            gateway_api_key_id: GatewayApiKeyId::new(),
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some(model.into()),
            thinking_level: None,
            provider_endpoint_id: None,
            credential_id: None,
            oauth_account_id: None,
            proxy_profile_id: None,
            attempt_count: 0,
            is_stream: None,
        }
    }
}
