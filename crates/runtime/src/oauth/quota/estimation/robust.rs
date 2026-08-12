use super::state::QuotaCapacitySample;

pub(super) fn median(samples: &[QuotaCapacitySample]) -> Option<f64> {
    median_values(
        samples
            .iter()
            .map(|sample| sample.capacity_credits)
            .collect(),
    )
}

/// Display-only dispersion diagnostic; classification is directional and does
/// not consume it (ADR-0143).
pub(super) fn relative_mad(samples: &[QuotaCapacitySample]) -> Option<f64> {
    let center = median(samples)?;
    if center <= 0.0 {
        return None;
    }
    let deviations = samples
        .iter()
        .map(|sample| (sample.capacity_credits - center).abs())
        .collect();
    Some(median_values(deviations)? / center)
}

fn median_values(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        Some((values[middle - 1] + values[middle]) / 2.0)
    } else {
        Some(values[middle])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(values: &[f64]) -> Vec<QuotaCapacitySample> {
        values
            .iter()
            .map(|value| QuotaCapacitySample {
                capacity_credits: *value,
                observed_at_ms: 0,
                epoch: 1,
                rate_cards: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn median_and_mad_are_robust_to_one_large_value() {
        let values = samples(&[990.0, 1_000.0, 1_010.0, 9_000.0]);
        assert_eq!(median(&values), Some(1_005.0));
        assert!(relative_mad(&values).is_some_and(|value| value < 0.02));
    }
}
