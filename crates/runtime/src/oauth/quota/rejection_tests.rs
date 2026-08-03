use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use any2api_domain::{ConfigRevision, ProviderKind};
use any2api_provider::api::OAuthProviderEgressStatus;
use tokio::sync::{Barrier, Notify};

use super::{EgressProbeCache, ProbeKey};

#[tokio::test]
async fn same_provider_revision_executes_once_and_reuses_the_cache() {
    let cache = Arc::new(EgressProbeCache::default());
    let key = ProbeKey {
        provider: ProviderKind::Codex,
        revision: ConfigRevision::INITIAL.get(),
    };
    let executions = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let cache = Arc::clone(&cache);
        let executions = Arc::clone(&executions);
        let started = Arc::clone(&started);
        let release = Arc::clone(&release);
        tasks.push(tokio::spawn(async move {
            cache
                .resolve(key, || async move {
                    executions.fetch_add(1, Ordering::SeqCst);
                    started.notify_one();
                    release.notified().await;
                    OAuthProviderEgressStatus::Restricted
                })
                .await
        }));
    }

    tokio::time::timeout(Duration::from_secs(1), started.notified())
        .await
        .expect("the single probe starts");
    tokio::task::yield_now().await;
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    release.notify_one();
    for task in tasks {
        assert_eq!(
            task.await.expect("probe waiter"),
            OAuthProviderEgressStatus::Restricted
        );
    }

    let cached = cache
        .resolve(key, || async {
            panic!("a fresh cached key must not execute another probe")
        })
        .await;
    assert_eq!(cached, OAuthProviderEgressStatus::Restricted);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn different_provider_revision_keys_probe_in_parallel() {
    let cache = Arc::new(EgressProbeCache::default());
    let initial = ConfigRevision::INITIAL;
    let next = initial.checked_next().expect("next revision");
    let keys = [
        ProbeKey {
            provider: ProviderKind::Codex,
            revision: initial.get(),
        },
        ProbeKey {
            provider: ProviderKind::Claude,
            revision: initial.get(),
        },
        ProbeKey {
            provider: ProviderKind::Codex,
            revision: next.get(),
        },
    ];
    let started = Arc::new(Barrier::new(keys.len() + 1));
    let release = Arc::new(Barrier::new(keys.len() + 1));
    let executions = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for key in keys {
        let cache = Arc::clone(&cache);
        let started = Arc::clone(&started);
        let release = Arc::clone(&release);
        let executions = Arc::clone(&executions);
        tasks.push(tokio::spawn(async move {
            cache
                .resolve(key, || async move {
                    executions.fetch_add(1, Ordering::SeqCst);
                    started.wait().await;
                    release.wait().await;
                    OAuthProviderEgressStatus::Reachable
                })
                .await
        }));
    }

    tokio::time::timeout(Duration::from_secs(1), started.wait())
        .await
        .expect("all independent probe keys start without waiting for one another");
    assert_eq!(executions.load(Ordering::SeqCst), keys.len());
    release.wait().await;
    for task in tasks {
        assert_eq!(
            task.await.expect("independent probe"),
            OAuthProviderEgressStatus::Reachable
        );
    }
}
