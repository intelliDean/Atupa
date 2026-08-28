//! Protocol diff structures: [`ProtocolDiffReport`] and [`DiffRow`].

use serde::{Deserialize, Serialize};

// ─── ProtocolDiffReport ───────────────────────────────────────────────────────

/// A field-by-field comparison report between two protocol executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolDiffReport {
    /// Human-readable name of the protocol being compared (e.g. `"Lido stETH"`).
    pub protocol: String,
    /// Ordered list of metric comparisons.
    pub rows: Vec<DiffRow>,
}

impl ProtocolDiffReport {
    /// Returns `true` if any row in this report represents a regression.
    pub fn has_regressions(&self) -> bool {
        self.rows.iter().any(DiffRow::is_regression)
    }

    /// Returns an iterator over only the rows that are regressions.
    pub fn regressions(&self) -> impl Iterator<Item = &DiffRow> {
        self.rows.iter().filter(|r| r.is_regression())
    }

    /// Returns an iterator over only the rows that are improvements.
    pub fn improvements(&self) -> impl Iterator<Item = &DiffRow> {
        self.rows.iter().filter(|r| r.is_improvement())
    }
}

// ─── DiffRow ──────────────────────────────────────────────────────────────────

/// A single comparable metric between a base and target execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffRow {
    /// Human-readable metric name (e.g. `"Total Gas"`).
    pub metric: String,
    /// Metric value for the base transaction.
    pub base: f64,
    /// Metric value for the target transaction.
    pub target: f64,
    /// Absolute difference: `target - base`.
    pub delta: f64,
    /// Percentage change relative to the base: `delta / base * 100`.
    ///
    /// Returns `0.0` when `base` is `0` to avoid division by zero.
    pub pct: f64,
    /// When `true`, an *increase* in this metric is a regression (e.g. gas cost, read count).
    /// When `false`, a *decrease* in this metric is a regression.
    pub higher_is_worse: bool,
}

impl DiffRow {
    /// Construct a new [`DiffRow`], automatically computing `delta` and `pct`.
    ///
    /// ```
    /// use atupa_core::DiffRow;
    ///
    /// let row = DiffRow::new("Total Gas", 1_000.0, 1_200.0, true);
    /// assert_eq!(row.delta, 200.0);
    /// assert_eq!(row.pct, 20.0);
    /// assert!(row.is_regression());
    /// ```
    pub fn new(metric: &str, base: f64, target: f64, higher_is_worse: bool) -> Self {
        let delta = target - base;
        let pct = if base != 0.0 { delta / base * 100.0 } else { 0.0 };
        Self {
            metric: metric.to_string(),
            base,
            target,
            delta,
            pct,
            higher_is_worse,
        }
    }

    /// Returns `true` if this metric has regressed (moved in the undesired direction).
    ///
    /// - `higher_is_worse = true` → regression when `delta > 0` (cost increased).
    /// - `higher_is_worse = false` → regression when `delta < 0` (a desirable metric decreased).
    pub fn is_regression(&self) -> bool {
        (self.higher_is_worse && self.delta > 0.0)
            || (!self.higher_is_worse && self.delta < 0.0)
    }

    /// Returns `true` if this metric has improved relative to the baseline.
    pub fn is_improvement(&self) -> bool {
        (self.higher_is_worse && self.delta < 0.0)
            || (!self.higher_is_worse && self.delta > 0.0)
    }

    /// Returns `true` if the metric value is unchanged between base and target.
    pub fn is_neutral(&self) -> bool {
        self.delta == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DiffRow ───────────────────────────────────────────────────────────────

    #[test]
    fn positive_delta_with_higher_is_worse_is_regression() {
        let row = DiffRow::new("Total Gas", 100.0, 150.0, true);
        assert_eq!(row.delta, 50.0);
        assert_eq!(row.pct, 50.0);
        assert!(row.is_regression());
        assert!(!row.is_improvement());
        assert!(!row.is_neutral());
    }

    #[test]
    fn negative_delta_with_higher_is_worse_is_improvement() {
        let row = DiffRow::new("Total Gas", 100.0, 80.0, true);
        assert_eq!(row.delta, -20.0);
        assert_eq!(row.pct, -20.0);
        assert!(row.is_improvement());
        assert!(!row.is_regression());
    }

    #[test]
    fn zero_base_pct_is_zero_not_nan() {
        let row = DiffRow::new("New Metric", 0.0, 42.0, true);
        assert_eq!(row.delta, 42.0);
        assert_eq!(row.pct, 0.0, "pct must be 0 when base is 0 to avoid NaN/inf");
    }

    #[test]
    fn unchanged_metric_is_neutral() {
        let row = DiffRow::new("Steps", 50.0, 50.0, true);
        assert!(row.is_neutral());
        assert!(!row.is_regression());
        assert!(!row.is_improvement());
    }

    #[test]
    fn lower_is_better_regression_when_delta_negative() {
        // E.g. "coverage %" where higher is better
        let row = DiffRow::new("Coverage %", 80.0, 70.0, false);
        assert!(row.is_regression());
        assert!(!row.is_improvement());
    }

    // ── ProtocolDiffReport ────────────────────────────────────────────────────

    #[test]
    fn report_detects_regressions() {
        let rows = vec![
            DiffRow::new("Gas", 100.0, 120.0, true),  // regression
            DiffRow::new("Steps", 50.0, 50.0, true),  // neutral
            DiffRow::new("Reads", 10.0, 8.0, true),   // improvement
        ];
        let report = ProtocolDiffReport { protocol: "Test".to_string(), rows };
        assert!(report.has_regressions());
        assert_eq!(report.regressions().count(), 1);
        assert_eq!(report.improvements().count(), 1);
    }

    #[test]
    fn report_with_no_regressions() {
        let rows = vec![
            DiffRow::new("Gas", 100.0, 90.0, true),   // improvement
            DiffRow::new("Steps", 50.0, 50.0, true),  // neutral
        ];
        let report = ProtocolDiffReport { protocol: "Test".to_string(), rows };
        assert!(!report.has_regressions());
        assert_eq!(report.regressions().count(), 0);
        assert_eq!(report.improvements().count(), 1);
    }
}
