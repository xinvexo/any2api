use std::collections::HashSet;

use any2api_domain::{OAuthAccountDraft, OAuthAccountId, ProviderKind};

use super::unique_label;

#[test]
fn truncated_label_is_trimmed_and_accepted_by_the_domain() {
    let mut used = HashSet::new();
    let preferred = format!("{} tail", "a".repeat(99));

    let label = unique_label(
        &mut used,
        ProviderKind::Codex,
        Some(&preferred),
        OAuthAccountId::new(),
    );

    assert_eq!(label, "a".repeat(99));
    assert_eq!(label.trim_end(), label);
    assert!(label.chars().count() <= 100);
    OAuthAccountDraft::new(label, None, true).expect("generated label must be valid");
}

#[test]
fn repeated_truncated_labels_remain_trimmed_and_unique() {
    let mut used = HashSet::new();
    let preferred = format!("{} tail", "a".repeat(99));

    let first = unique_label(
        &mut used,
        ProviderKind::Codex,
        Some(&preferred),
        OAuthAccountId::new(),
    );
    let second = unique_label(
        &mut used,
        ProviderKind::Codex,
        Some(&preferred),
        OAuthAccountId::new(),
    );

    assert_eq!(first, "a".repeat(99));
    assert_eq!(second, format!("{} (2)", "a".repeat(96)));
    assert_eq!(second.chars().count(), 100);
    OAuthAccountDraft::new(second, None, true).expect("deduplicated label must be valid");
}
