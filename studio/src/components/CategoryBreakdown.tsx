import { CATEGORY_META, fmtGas } from '../types/trace';
import type { StitchedReport, GasCategory } from '../types/trace';

interface Props {
  report: StitchedReport;
}

export function CategoryBreakdown({ report }: Props) {
  const categories = Object.entries(report.category_costs) as [GasCategory, number][];
  // Filter out zero costs and sort by value
  const sorted = categories
    .filter(([_, gas]) => gas > 0)
    .sort((a, b) => b[1] - a[1]);
  
  const total = report.total_unified_cost || 1;

  return (
    <div className="category-breakdown">
      <div className="category-chart">
        {sorted.map(([cat, gas]) => {
          const pct = (gas / total) * 100;
          const meta = CATEGORY_META[cat];
          return (
            <div 
              key={cat}
              className="category-slice"
              style={{ 
                width: `${pct}%`, 
                backgroundColor: meta.color,
              }}
              title={`${meta.label}: ${fmtGas(gas)} gas (${pct.toFixed(1)}%)`}
            />
          );
        })}
      </div>

      <div className="category-legend">
        {sorted.map(([cat, gas]) => {
          const pct = (gas / total) * 100;
          const meta = CATEGORY_META[cat];
          return (
            <div key={cat} className="legend-item">
              <span className="legend-dot" style={{ backgroundColor: meta.color }} />
              <span className="legend-icon">{meta.icon}</span>
              <span className="legend-label">{meta.label}</span>
              <div className="legend-spacer" />
              <span className="legend-value">{fmtGas(Math.round(gas))} gas</span>
              <span className="legend-pct">{pct.toFixed(1)}%</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
