//! [`GhoSupplyMetrics`] and the per-label classification helper.

use serde::{Deserialize, Serialize};

/// Aggregated GHO supply-level metrics extracted from trace steps.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GhoSupplyMetrics {
    /// Number of `mint` calls observed in the trace.
    pub mint_count: u32,
    /// Number of `burn` calls observed in the trace.
    pub burn_count: u32,
    /// Number of `updateFacilitatorBucketCapacity` calls (risk signal).
    pub bucket_capacity_updates: u32,
    /// Number of `distributeFeesToTreasury` calls.
    pub fee_distributions: u32,
}

/// Update [`GhoSupplyMetrics`] for a single recognized GHO label.
pub(crate) fn classify_gho_label(label: &str, metrics: &mut GhoSupplyMetrics) {
    match label {
        "GHO::mint" => metrics.mint_count += 1,
        "GHO::burn" => metrics.burn_count += 1,
        "GHO::updateFacilitatorBucketCapacity" => metrics.bucket_capacity_updates += 1,
        "GHO::distributeFeesToTreasury" => metrics.fee_distributions += 1,
        _ => {}
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_mint_increments_mint_count() {
        let mut m = GhoSupplyMetrics::default();
        classify_gho_label("GHO::mint", &mut m);
        classify_gho_label("GHO::mint", &mut m);
        assert_eq!(m.mint_count, 2);
        assert_eq!(m.burn_count, 0);
    }

    #[test]
    fn classify_burn_increments_burn_count() {
        let mut m = GhoSupplyMetrics::default();
        classify_gho_label("GHO::burn", &mut m);
        assert_eq!(m.burn_count, 1);
    }

    #[test]
    fn classify_bucket_capacity_update() {
        let mut m = GhoSupplyMetrics::default();
        classify_gho_label("GHO::updateFacilitatorBucketCapacity", &mut m);
        assert_eq!(m.bucket_capacity_updates, 1);
    }

    #[test]
    fn classify_fee_distribution() {
        let mut m = GhoSupplyMetrics::default();
        classify_gho_label("GHO::distributeFeesToTreasury", &mut m);
        assert_eq!(m.fee_distributions, 1);
    }

    #[test]
    fn classify_unknown_label_is_noop() {
        let mut m = GhoSupplyMetrics::default();
        classify_gho_label("AaveV3Pool::supply", &mut m);
        classify_gho_label("unknown", &mut m);
        assert_eq!(m, GhoSupplyMetrics::default());
    }
}
