use super::*;

#[tokio::test(start_paused = true)]
async fn fixed_waiters_receive_the_next_expired_rpm_slot_first() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let bindings = reconcile(
        &runtime,
        fixture.configuration(Some(1), 1, 1),
        "sk-fixed-test",
    );
    let binding = bindings.as_slice()[0].clone();
    drop(binding.try_reserve().expect("initial reservation"));
    let fixed_waiter = binding.register_fixed_waiter();

    assert!(binding.try_reserve().is_err());
    assert!(binding.try_reserve_fixed().is_err());
    tokio::time::advance(Duration::from_secs(60)).await;
    assert!(binding.try_reserve().is_err());
    drop(
        binding
            .try_reserve_fixed()
            .expect("fixed waiter reservation"),
    );

    drop(fixed_waiter);
    assert!(binding.try_reserve().is_err());
}

#[test]
fn fixed_waiters_reserve_only_the_capacity_they_need() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let bindings = reconcile(
        &runtime,
        fixture.configuration(Some(3), 1, 1),
        "sk-fixed-capacity",
    );
    let binding = bindings.as_slice()[0].clone();
    let first = binding.try_reserve().expect("first normal reservation");
    let fixed_waiter = binding.register_fixed_waiter();

    let second = binding
        .try_reserve()
        .expect("normal request may use capacity not reserved by the fixed waiter");
    assert!(binding.try_reserve().is_err());
    let fixed = binding
        .try_reserve_fixed()
        .expect("reserved fixed-waiter capacity");

    drop((first, second, fixed, fixed_waiter));
}

#[tokio::test]
async fn fixed_waiter_registration_notifies_its_credential_without_a_global_epoch() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let bindings = reconcile(
        &runtime,
        fixture.configuration(Some(1), 1, 1),
        "sk-fixed-notify",
    );
    let binding = bindings.as_slice()[0].clone();
    let mut changes = binding.subscribe_changes();
    let epoch_before = runtime.scheduler_epoch();
    let _observed = *changes.borrow_and_update();

    let fixed_waiter = binding.register_fixed_waiter();
    assert_eq!(runtime.scheduler_epoch(), epoch_before);
    assert!(changes.has_changed().expect("change channel open"));
    let _observed = *changes.borrow_and_update();

    drop(fixed_waiter);
    assert!(changes.has_changed().expect("change channel open"));
    assert_eq!(runtime.scheduler_epoch(), epoch_before + 1);
}
