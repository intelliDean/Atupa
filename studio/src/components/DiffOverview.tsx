import { fmtGas, fmtEquiv } from '../types/trace';
import type { DiffReport } from '../types/trace';
import { MetricCard } from './MetricCard';
import { CategoryBreakdown } from './CategoryBreakdown';

interface Props {
  report: DiffReport;
}

export function DiffOverview({ report }: Props) {
  const { base, target, metrics } = report;

  const DeltaLabel = ({ val, pct }: { val: number; pct: number }) => {
    const isIncrease = val > 0;
    const color = isIncrease ? '#ff4d4d' : '#4dff88';
    const sign = isIncrease ? '+' : '';
    return (
      <span style={{ color, fontWeight: 'bold', marginLeft: 8 }}>
        {sign}{fmtGas(Math.round(val))} ({sign}{pct.toFixed(1)}%)
      </span>
    );
  };

  return (
    <div className="diff-overview">
      {/* ── Summary Header ─────────────────────────────────────────────────── */}
      <div className="glass-card">
        <div className="section-header">
          <span className="section-title">📊 Comparison Summary</span>
          <div className="section-divider" />
        </div>
        <div className="metric-grid">
          <MetricCard
            kind="evm"
            icon="⛽"
            label="On-Chain Gas"
            value={fmtGas(metrics.target_total_gas)}
            sub={
              <>
                Baseline: {fmtGas(metrics.base_total_gas)}
                <DeltaLabel val={metrics.gas_delta} pct={metrics.gas_pct} />
              </>
            }
          />
          <MetricCard
            kind="stylus"
            icon="🦾"
            label="Execution Cost (Unified)"
            value={fmtEquiv(metrics.target_unified_cost)}
            sub={
              <>
                Baseline: {fmtEquiv(metrics.base_unified_cost)}
                <DeltaLabel val={metrics.unified_delta} pct={metrics.unified_pct} />
              </>
            }
          />
        </div>
      </div>

      {/* ── Side-by-Side Breakdown ────────────────────────────────────────── */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 20 }}>
        <div className="glass-card">
          <div className="section-header">
            <span className="section-title">📐 Baseline ( {base.tx_hash.slice(0, 8)}… )</span>
            <div className="section-divider" />
          </div>
          <CategoryBreakdown report={base} />
        </div>
        <div className="glass-card">
          <div className="section-header">
            <span className="section-title">🎯 Target ( {target.tx_hash.slice(0, 8)}… )</span>
            <div className="section-divider" />
          </div>
          <CategoryBreakdown report={target} />
        </div>
      </div>
    </div>
  );
}
