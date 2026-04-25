import type { ReactNode } from 'react';

interface Props {
  label: string;
  value: string;
  sub?: ReactNode;
  kind: 'evm' | 'stylus' | 'boundary' | 'steps';
  icon?: string;
}

export function MetricCard({ label, value, sub, kind, icon }: Props) {
  return (
    <div className={`metric-card ${kind}`} role="region" aria-label={label}>
      <div className="metric-label">
        {icon && <span style={{ marginRight: 5 }}>{icon}</span>}
        {label}
      </div>
      <div className={`metric-value ${kind}`}>{value}</div>
      {sub && <div className="metric-sub">{sub}</div>}
    </div>
  );
}
