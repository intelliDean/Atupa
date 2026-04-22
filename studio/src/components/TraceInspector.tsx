import React, { useState, useMemo } from 'react';
import { getDisplayLabel } from '../types/trace';
import type { StitchedReport, UnifiedStep } from '../types/trace';

interface Props {
  report: StitchedReport;
}

const PAGE_SIZE = 150;

function StepRow({ step, report }: { step: UnifiedStep, report: StitchedReport }) {
  const indent = Array.from({ length: Math.max(0, step.depth - 1) }).map((_, i) => (
    <span key={i} className="trace-depth-indent" />
  ));

  const costStr = step.vm === 'Evm'
    ? step.gas_cost > 0 ? `${step.gas_cost} gas` : ''
    : `${step.cost_equiv.toFixed(2)} gas-equiv`;

  const displayLabel = getDisplayLabel(step, report);
  const isResolved = step.target_address && report.resolved_names[step.target_address];

  return (
    <div
      className={`trace-step${step.is_vm_boundary ? ' is-boundary' : ''}`}
      role="listitem"
      title={isResolved ? `Target: ${step.target_address}` : (step.is_vm_boundary ? 'EVM→WASM Boundary Crossing' : undefined)}
    >
      <span className="trace-step-index">#{step.index}</span>
      {indent}
      <span className={`trace-step-badge ${step.vm === 'Evm' ? 'evm' : 'stylus'}`}>
        {step.vm === 'Evm' ? 'EVM' : 'WASM'}
      </span>
      <span className={`trace-step-label ${isResolved ? 'resolved' : ''}`}>{displayLabel}</span>
      {costStr && <span className="trace-step-cost">{costStr}</span>}
      {step.is_vm_boundary && (
        <span style={{ fontSize: 10, color: 'var(--color-violet)', marginLeft: 4 }}>⇌</span>
      )}
    </div>
  );
}

export function TraceInspector({ report }: Props) {
  const [filter, setFilter] = useState<'all' | 'evm' | 'stylus' | 'boundary'>('all');
  const [page, setPage] = useState(0);
  const [search, setSearch] = useState('');

  const filtered = useMemo(() => {
    return report.steps.filter((s: UnifiedStep) => {
      if (filter === 'evm' && s.vm !== 'Evm') return false;
      if (filter === 'stylus' && s.vm !== 'Stylus') return false;
      if (filter === 'boundary' && !s.is_vm_boundary) return false;
      
      const label = getDisplayLabel(s, report).toLowerCase();
      if (search && !label.includes(search.toLowerCase())) return false;
      return true;
    });
  }, [report.steps, filter, search]);

  const pageCount = Math.ceil(filtered.length / PAGE_SIZE);
  const visible = filtered.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);

  const chipStyle = (active: boolean) => ({
    padding: '4px 12px',
    borderRadius: 99,
    fontSize: 11,
    fontWeight: 600,
    cursor: 'pointer',
    border: `1px solid ${active ? 'var(--color-border-accent)' : 'var(--color-border)'}`,
    background: active ? 'var(--color-crimson-glow)' : 'transparent',
    color: active ? 'var(--color-crimson)' : 'var(--color-text-secondary)',
    transition: 'all 150ms',
  } as React.CSSProperties);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-3)' }}>
      {/* Controls */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-3)', flexWrap: 'wrap' }}>
        {(['all', 'evm', 'stylus', 'boundary'] as const).map((f) => (
          <button
            key={f}
            id={`filter-${f}`}
            style={chipStyle(filter === f)}
            onClick={() => { setFilter(f); setPage(0); }}
          >
            {f === 'all' ? 'All Steps' : f === 'evm' ? 'EVM Only' : f === 'stylus' ? 'WASM Only' : 'Boundaries'}
          </button>
        ))}

        <input
          id="trace-search"
          type="search"
          placeholder="Search opcode / HostIO…"
          value={search}
          onChange={(e) => { setSearch(e.target.value); setPage(0); }}
          style={{
            marginLeft: 'auto',
            padding: '5px 12px',
            background: 'var(--color-bg-raised)',
            border: '1px solid var(--color-border)',
            borderRadius: 6,
            color: 'var(--color-text-primary)',
            fontSize: 12,
            fontFamily: 'var(--font-mono)',
            outline: 'none',
            width: 220,
          }}
        />
      </div>

      {/* Step count */}
      <div style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)' }}>
        Showing {visible.length} of {filtered.length} steps
        {pageCount > 1 && ` (page ${page + 1}/${pageCount})`}
      </div>

      {/* Steps list */}
      <div className="trace-list glass-card" role="list" style={{ maxHeight: 480, overflowY: 'auto', padding: 'var(--sp-3)' }}>
        {visible.length === 0
          ? <div style={{ color: 'var(--color-text-muted)', fontSize: 13, padding: 'var(--sp-4)' }}>No steps match your filter.</div>
          : visible.map((s) => <StepRow key={s.index} step={s} report={report} />)
        }
      </div>

      {/* Pagination */}
      {pageCount > 1 && (
        <div style={{ display: 'flex', gap: 'var(--sp-2)', alignItems: 'center', justifyContent: 'center' }}>
          <button
            id="page-prev"
            onClick={() => setPage((p) => Math.max(0, p - 1))}
            disabled={page === 0}
            style={chipStyle(false)}
          >
            ← Prev
          </button>
          <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>{page + 1} / {pageCount}</span>
          <button
            id="page-next"
            onClick={() => setPage((p) => Math.min(pageCount - 1, p + 1))}
            disabled={page === pageCount - 1}
            style={chipStyle(false)}
          >
            Next →
          </button>
        </div>
      )}
    </div>
  );
}
