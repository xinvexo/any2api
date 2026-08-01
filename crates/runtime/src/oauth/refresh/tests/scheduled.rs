use super::*;

#[tokio::test]
async fn scheduled_refreshes_use_bounded_network_concurrency_and_one_revision() {
    const ACCOUNT_COUNT: usize = 7;
    let transport = Arc::new(ConcurrentRefreshTransport::success());
    let context = RefreshTestContext::with_accounts(transport.clone(), ACCOUNT_COUNT).await;
    let before = context.snapshots.load().revision();

    let refresher = Arc::clone(&context.refresher);
    let scan = tokio::spawn(async move { refresher.scan_due_accounts().await });
    transport.wait_until_started(6).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(transport.calls(), 6);
    assert_eq!(transport.max_in_flight(), 6);

    transport.release(6);
    transport.wait_until_started(1).await;
    transport.release(1);
    scan.await.expect("scheduled refresh scan");

    assert_eq!(transport.calls(), ACCOUNT_COUNT);
    assert_eq!(transport.max_in_flight(), 6);
    let published = context.snapshots.load();
    assert_eq!(published.revision().get(), before.get() + 1);
    for id in context.account_ids {
        assert_eq!(
            published
                .oauth_accounts()
                .get(id)
                .expect("refreshed account")
                .token_version(),
            2
        );
    }
}

#[tokio::test]
async fn scheduled_refresh_batch_keeps_successes_when_one_network_request_fails() {
    let transport = Arc::new(ConcurrentRefreshTransport::failing_call(0));
    let context = RefreshTestContext::with_accounts(transport.clone(), 2).await;
    let before = context.snapshots.load().revision();
    transport.release(2);

    context.refresher.scan_due_accounts().await;

    let published = context.snapshots.load();
    assert_eq!(published.revision().get(), before.get() + 1);
    assert_eq!(transport.calls(), 2);
    let mut versions = context
        .account_ids
        .iter()
        .map(|id| {
            published
                .oauth_accounts()
                .get(*id)
                .expect("OAuth account")
                .token_version()
        })
        .collect::<Vec<_>>();
    versions.sort_unstable();
    assert_eq!(versions, vec![1, 2]);
}

#[tokio::test]
async fn cancelling_scan_waiter_keeps_singleflight_until_detached_publish_finishes() {
    let transport = Arc::new(ConcurrentRefreshTransport::success());
    let context = RefreshTestContext::with_accounts(transport.clone(), 1).await;
    let account_id = context.account_ids[0];
    let before = context.snapshots.load().revision();
    let publish_guard = context.snapshots.acquire_publish().await;

    let refresher = Arc::clone(&context.refresher);
    let scan = tokio::spawn(async move { refresher.scan_due_accounts().await });
    transport.wait_until_started(1).await;
    transport.release(1);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while context._runtime.lifecycle().background_task_count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("detached publication task");

    scan.abort();
    assert!(
        scan.await
            .expect_err("scan waiter must be cancelled")
            .is_cancelled()
    );

    let refresher = Arc::clone(&context.refresher);
    let waiter = tokio::spawn(async move { refresher.refresh_if_due(account_id, 1).await });
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(transport.calls(), 1);

    drop(publish_guard);
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("singleflight waiter completion")
            .expect("singleflight waiter")
            .expect("refresh result"),
        None
    );
    assert_eq!(transport.calls(), 1);
    assert_eq!(context.snapshots.load().revision().get(), before.get() + 1);
}

#[tokio::test]
async fn own_revision_does_not_hot_loop_when_expiry_stays_due() {
    let transport = Arc::new(ConcurrentRefreshTransport::with_response(
        Bytes::from_static(br#"{"access_token":"new-access"}"#),
    ));
    let context = RefreshTestContext::with_accounts(transport.clone(), 1).await;
    let account_id = context.account_ids[0];
    let lifecycle = context._runtime.lifecycle();
    transport.release(2);
    tokio::time::pause();

    assert!(context.refresher.start(&lifecycle));
    transport.wait_until_started(1).await;
    for _ in 0..100 {
        if context
            .snapshots
            .load()
            .oauth_accounts()
            .get(account_id)
            .expect("OAuth account")
            .token_version()
            == 2
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(transport.calls(), 1);

    tokio::time::advance(std::time::Duration::from_secs(29)).await;
    tokio::task::yield_now().await;
    assert_eq!(transport.calls(), 1);

    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    transport.wait_until_started(1).await;
    assert_eq!(transport.calls(), 2);

    lifecycle.force();
    lifecycle.close_background_tasks();
    lifecycle.wait_for_background_tasks().await;
}

struct ConcurrentRefreshTransport {
    started: Semaphore,
    release: Semaphore,
    calls: AtomicUsize,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
    failing_call: Option<usize>,
    response: Bytes,
}

impl ConcurrentRefreshTransport {
    fn success() -> Self {
        Self::new(
            None,
            Bytes::from_static(br#"{"access_token":"new-access","expires_in":3600}"#),
        )
    }

    fn failing_call(call: usize) -> Self {
        Self::new(
            Some(call),
            Bytes::from_static(br#"{"access_token":"new-access","expires_in":3600}"#),
        )
    }

    fn with_response(response: Bytes) -> Self {
        Self::new(None, response)
    }

    fn new(failing_call: Option<usize>, response: Bytes) -> Self {
        Self {
            started: Semaphore::new(0),
            release: Semaphore::new(0),
            calls: AtomicUsize::new(0),
            in_flight: AtomicUsize::new(0),
            max_in_flight: AtomicUsize::new(0),
            failing_call,
            response,
        }
    }

    async fn wait_until_started(&self, count: usize) {
        self.started
            .acquire_many(u32::try_from(count).expect("test count fits u32"))
            .await
            .expect("refresh start signal")
            .forget();
    }

    fn release(&self, count: usize) {
        self.release.add_permits(count);
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }

    fn max_in_flight(&self) -> usize {
        self.max_in_flight.load(Ordering::Acquire)
    }
}

#[async_trait]
impl TransportManager for ConcurrentRefreshTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        _request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        let in_flight = self.in_flight.fetch_add(1, Ordering::AcqRel) + 1;
        self.max_in_flight.fetch_max(in_flight, Ordering::AcqRel);
        self.started.add_permits(1);
        self.release
            .acquire()
            .await
            .expect("refresh release signal")
            .forget();
        self.in_flight.fetch_sub(1, Ordering::AcqRel);

        let body: BoxByteStream = Box::pin(stream::iter([Ok(self.response.clone())]));
        Ok(TransportResponse {
            status: if self.failing_call == Some(call) {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::OK
            },
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}
