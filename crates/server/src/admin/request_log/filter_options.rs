use any2api_domain::RequestLog;
use any2api_runtime::api::PublishedSnapshot;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct RequestLogFilterOptionsResponse {
    public_models: Vec<String>,
    gateway_api_keys: Vec<StableFilterOption>,
    provider_credentials: Vec<StableFilterOption>,
    oauth_accounts: Vec<StableFilterOption>,
}

impl RequestLogFilterOptionsResponse {
    pub(super) fn new(logs: &[RequestLog], snapshot: &PublishedSnapshot) -> Self {
        let mut public_models = snapshot.public_model_names();
        public_models.extend(logs.iter().filter_map(|log| log.public_model.clone()));

        let mut gateway_api_keys = snapshot
            .gateway_api_keys()
            .keys()
            .iter()
            .map(|key| StableFilterOption::active(key.id().to_string(), key.name().to_owned()))
            .collect::<Vec<_>>();
        let mut provider_credentials = snapshot
            .provider_credentials()
            .credentials()
            .iter()
            .map(|credential| {
                let endpoint = snapshot
                    .provider_endpoints()
                    .get(credential.provider_endpoint_id())
                    .expect("published credential endpoint exists");
                StableFilterOption::active(
                    credential.id().to_string(),
                    format!("{} / {}", endpoint.name(), credential.label()),
                )
            })
            .collect::<Vec<_>>();
        let mut oauth_accounts = snapshot
            .oauth_accounts()
            .accounts()
            .iter()
            .map(|account| {
                StableFilterOption::active(
                    account.id().to_string(),
                    format!("{} / {}", account.provider_kind().as_str(), account.label()),
                )
            })
            .collect::<Vec<_>>();

        for log in logs {
            if let Some(id) = log.gateway_api_key_id {
                push_deleted_option(&mut gateway_api_keys, id.to_string());
            }
            if let Some(id) = log.credential_id {
                push_deleted_option(&mut provider_credentials, id.to_string());
            }
            if let Some(id) = log.oauth_account_id {
                push_deleted_option(&mut oauth_accounts, id.to_string());
            }
        }
        sort_options(&mut gateway_api_keys);
        sort_options(&mut provider_credentials);
        sort_options(&mut oauth_accounts);

        Self {
            public_models: public_models.into_iter().collect(),
            gateway_api_keys,
            provider_credentials,
            oauth_accounts,
        }
    }
}

#[derive(Serialize)]
struct StableFilterOption {
    id: String,
    label: String,
    deleted: bool,
}

impl StableFilterOption {
    fn active(id: String, label: String) -> Self {
        Self {
            id,
            label,
            deleted: false,
        }
    }
}

fn push_deleted_option(options: &mut Vec<StableFilterOption>, id: String) {
    if options.iter().any(|option| option.id == id) {
        return;
    }
    let label = id.chars().take(8).collect();
    options.push(StableFilterOption {
        id,
        label,
        deleted: true,
    });
}

fn sort_options(options: &mut [StableFilterOption]) {
    options.sort_by(|left, right| {
        left.deleted
            .cmp(&right.deleted)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });
}

#[cfg(test)]
mod tests {
    use super::{StableFilterOption, push_deleted_option};

    #[test]
    fn deleted_options_are_only_added_when_the_snapshot_option_is_absent() {
        let id = "11111111-1111-4111-8111-111111111111".to_owned();
        let mut options = vec![StableFilterOption::active(id.clone(), "Primary".into())];
        push_deleted_option(&mut options, id);
        assert_eq!(options.len(), 1);

        push_deleted_option(&mut options, "22222222-2222-4222-8222-222222222222".into());
        assert_eq!(options.len(), 2);
        assert!(options[1].deleted);
        assert_eq!(options[1].label, "22222222");
    }
}
