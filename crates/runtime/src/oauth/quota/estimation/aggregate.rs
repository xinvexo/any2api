use super::state::QuotaCapacitySample;

/// Robust center of the capacity samples, weighted by each sample's Δused%.
/// Larger official deltas dilute percent quantization and accounting-timing
/// skew, so their capacities are more trustworthy; the median form keeps a
/// single wild sample from dragging the estimate.
pub(super) fn weighted_median(samples: &[QuotaCapacitySample]) -> Option<f64> {
    let mut points = samples
        .iter()
        .map(|sample| (sample.capacity_credits, sample.delta_used_percent))
        .collect::<Vec<_>>();
    if points.is_empty()
        || points.iter().any(|(capacity, weight)| {
            !capacity.is_finite() || !weight.is_finite() || *weight <= 0.0
        })
    {
        return None;
    }
    points.sort_by(|left, right| f64::total_cmp(&left.0, &right.0));
    let total: f64 = points.iter().map(|(_, weight)| weight).sum();
    let half = total / 2.0;
    let mut cumulative = 0.0;
    for (index, (capacity, weight)) in points.iter().enumerate() {
        cumulative += weight;
        if cumulative > half {
            return Some(*capacity);
        }
        if cumulative == half {
            // Exact boundary: agree with the plain median under equal weights.
            let next = points.get(index + 1).map_or(*capacity, |point| point.0);
            return Some((capacity + next) / 2.0);
        }
    }
    points.last().map(|(capacity, _)| *capacity)
}

/// Relative dispersion of the samples around the weighted-median center; a
/// display and confidence diagnostic, never a gate on accepting samples.
pub(super) fn relative_mad(samples: &[QuotaCapacitySample]) -> Option<f64> {
    let center = weighted_median(samples)?;
    if center <= 0.0 {
        return None;
    }
    let mut deviations = samples
        .iter()
        .map(|sample| (sample.capacity_credits - center).abs())
        .collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    let middle = deviations.len() / 2;
    let mad = if deviations.len().is_multiple_of(2) {
        (deviations[middle - 1] + deviations[middle]) / 2.0
    } else {
        deviations[middle]
    };
    Some(mad / center)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(values: &[(f64, f64)]) -> Vec<QuotaCapacitySample> {
        values
            .iter()
            .map(|(capacity, delta)| QuotaCapacitySample {
                capacity_credits: *capacity,
                delta_used_percent: *delta,
                local_cost_credits: capacity * delta / 100.0,
                observed_at_ms: 0,
                epoch: 1,
                rate_cards: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn equal_weights_match_the_plain_median() {
        let odd = samples(&[(990.0, 5.0), (1_000.0, 5.0), (1_010.0, 5.0)]);
        assert_eq!(weighted_median(&odd), Some(1_000.0));
        let even = samples(&[(990.0, 5.0), (1_000.0, 5.0), (1_010.0, 5.0), (1_020.0, 5.0)]);
        assert_eq!(weighted_median(&even), Some(1_005.0));
    }

    #[test]
    fn heavier_samples_pull_the_center() {
        let values = samples(&[(16.0, 2.0), (15.0, 10.0), (16.5, 3.0)]);
        assert_eq!(weighted_median(&values), Some(15.0));
    }

    #[test]
    fn median_is_robust_to_one_large_value() {
        let values = samples(&[(990.0, 5.0), (1_000.0, 5.0), (1_010.0, 5.0), (9_000.0, 5.0)]);
        assert_eq!(weighted_median(&values), Some(1_005.0));
        assert!(relative_mad(&values).is_some_and(|value| value < 0.02));
    }
}
