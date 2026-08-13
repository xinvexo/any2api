use any2api_domain::{CodexQuotaRateCard, ConfigRevision, SettingKey, SettingValue};

use super::TestContext;
use crate::configuration::ConfigPublishError;

#[tokio::test]
async fn codex_rate_card_content_changes_require_a_new_id() {
    let context = TestContext::new().await;
    let initial = context.snapshots.load();
    let mut changed = CodexQuotaRateCard {
        credits_per_usd: 30,
        ..CodexQuotaRateCard::default()
    };

    let rejected = context
        .publisher
        .set_setting_override(
            ConfigRevision::INITIAL,
            SettingKey::OAuthCodexRateCard,
            SettingValue::CodexRateCard(changed.clone()),
        )
        .await
        .expect_err("same card ID must not change meaning");
    assert!(matches!(rejected, ConfigPublishError::InvalidSetting(_)));

    changed.id = "openai_codex_credits_2026_08_13".to_owned();
    let published = context
        .publisher
        .set_setting_override(
            ConfigRevision::INITIAL,
            SettingKey::OAuthCodexRateCard,
            SettingValue::CodexRateCard(changed),
        )
        .await
        .expect("versioned rate card");
    assert_eq!(
        published
            .settings()
            .oauth()
            .codex_rate_card()
            .credits_per_usd(),
        30
    );
    assert_eq!(
        initial
            .settings()
            .oauth()
            .codex_rate_card()
            .credits_per_usd(),
        25
    );
}
