use super::*;

#[tokio::test]
async fn one_slow_refresh_does_not_delay_ready_accounts_or_hold_their_gates() {
    let transport = Arc::new(OneSlowRefreshTransport::new());
    let context = RefreshTestContext::with_accounts(transport.clone(), 2).await;
    let before = context.snapshots.load().revision();

    let refresher = Arc::clone(&context.refresher);
    let scan = tokio::spawn(async move { refresher.scan_due_accounts().await });
    transport.wait_until_started(2).await;

    let first_published = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let snapshot = context.snapshots.load();
            if snapshot
                .oauth_accounts()
                .accounts()
                .iter()
                .any(|account| account.token_version() == 2)
            {
                break snapshot;
            }
            drop(snapshot);
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("a ready account must publish without the slow peer");
    assert_eq!(first_published.revision().get(), before.get() + 1);
    let mut first_versions = context
        .account_ids
        .iter()
        .map(|id| {
            first_published
                .oauth_accounts()
                .get(*id)
                .expect("OAuth account")
                .token_version()
        })
        .collect::<Vec<_>>();
    first_versions.sort_unstable();
    assert_eq!(first_versions, vec![1, 2]);
    drop(first_published);

    let refreshed_id = context
        .account_ids
        .iter()
        .copied()
        .find(|id| {
            context
                .snapshots
                .load()
                .oauth_accounts()
                .get(*id)
                .is_some_and(|account| account.token_version() == 2)
        })
        .expect("ready account");
    assert_eq!(
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            context.refresher.refresh_if_due(refreshed_id, 1),
        )
        .await
        .expect("published account gate must be released")
        .expect("already-refreshed result"),
        None
    );

    transport.release_slow();
    scan.await.expect("scheduled scan");
    let published = context.snapshots.load();
    assert_eq!(published.revision().get(), before.get() + 2);
    assert!(context.account_ids.iter().all(|id| {
        published
            .oauth_accounts()
            .get(*id)
            .is_some_and(|account| account.token_version() == 2)
    }));
}

struct OneSlowRefreshTransport {
    started: Semaphore,
    slow_release: Semaphore,
    calls: AtomicUsize,
}

impl OneSlowRefreshTransport {
    fn new() -> Self {
        Self {
            started: Semaphore::new(0),
            slow_release: Semaphore::new(0),
            calls: AtomicUsize::new(0),
        }
    }

    async fn wait_until_started(&self, count: usize) {
        self.started
            .acquire_many(u32::try_from(count).expect("test count fits u32"))
            .await
            .expect("refresh start signal")
            .forget();
    }

    fn release_slow(&self) {
        self.slow_release.add_permits(1);
    }
}

#[async_trait]
impl TransportManager for OneSlowRefreshTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        _request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        self.started.add_permits(1);
        if call == 0 {
            self.slow_release
                .acquire()
                .await
                .expect("slow refresh release")
                .forget();
        }
        let body: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
            br#"{"access_token":"new-access","expires_in":3600}"#,
        ))]));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}
