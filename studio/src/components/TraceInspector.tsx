import React, { useState, useMemo } from 'react';
import { getDisplayLabel } from '../types/trace';
import type { StitchedReport, UnifiedStep, VmKind } from '../types/trace';

interface Props {
  report: StitchedReport;
}

const PAGE_SIZE = 150;

function formatStepCost(step: UnifiedStep): string {
  if (step.vm === 'Solana') {
    return step.cost_equiv > 0 ? `${step.cost_equiv} CU` : '';
  }
  if (step.vm === 'Starknet') {
    return step.cost_equiv > 0 ? `${step.cost_equiv} steps` : '';
  }
  if (step.vm === 'Stellar') {
    return step.cost_equiv > 0 ? `${step.cost_equiv} units` : '';
  }
  if (step.vm === 'Evm') {
    return step.gas_cost > 0 ? `${step.gas_cost} gas` : '';
  }
  if (step.vm === 'Stylus') {
    return step.cost_equiv > 0 ? `${step.cost_equiv.toFixed(1)} gas-equiv` : '';
  }
  return step.cost_equiv > 0 ? `${step.cost_equiv}` : '';
}

function getBadgeLabel(vm: VmKind): { text: string; className: string } {
  switch (vm) {
    case 'Evm':      return { text: 'EVM', className: 'evm' };
    case 'Stylus':   return { text: 'WASM', className: 'stylus' };
    case 'Solana':   return { text: 'SVM', className: 'solana' };
    case 'Starknet': return { text: 'CAIRO', className: 'starknet' };
    case 'Stellar':  return { text: 'SOROBAN', className: 'stellar' };
  }
}

function StepRow({ step, report }: { step: UnifiedStep; report: StitchedReport }) {
  const indent = Array.from({ length: Math.max(0, step.depth - 1) }).map((_, i) => (
    <span key={i} className="trace-depth-indent" />
  ));

  const costStr = formatStepCost(step);
  const displayLabel = getDisplayLabel(step, report);
  const isResolved = step.target_address && report.resolved_names[step.target_address];
  const badge = getBadgeLabel(step.vm);

  return (
    <div
      className={`trace-step${step.is_vm_boundary ? ' is-boundary' : ''}`}
      role="listitem"
      title={isResolved ? `Target: ${step.target_address}` : (step.is_vm_boundary ? 'Cross-VM / CPI Boundary Crossing' : undefined)}
    >
      <span className="trace-step-index">#{step.index}</span>
      {indent}
      <span className={`trace-step-badge ${badge.className}`}>
        {badge.text}
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
  const presentVms = useMemo(() => {
    const set = new Set<VmKind>();
    for (const s of report.steps) set.add(s.vm);
    return Array.from(set);
  }, [report]);

  const [filter, setFilter] = useState<string>('all');
  const [page, setPage] = useState(0);
  const [search, setSearch] = useState('');

  const filtered = useMemo(() => {
    return report.steps.filter((s: UnifiedStep) => {
      if (filter === 'boundary') {
        if (!s.is_vm_boundary) return false;
      } else if (filter !== 'all') {
        if (s.vm !== filter) return false;
      }
      
      const label = getDisplayLabel(s, report).toLowerCase();
      if (search && !label.includes(search.toLowerCase())) return false;
      return true;
    });
  }, [report, filter, search]);

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
        <button
          id="filter-all"
          style={chipStyle(filter === 'all')}
          onClick={() => { setFilter('all'); setPage(0); }}
        >
          All Steps ({report.steps.length})
        </button>

        {presentVms.length > 1 && presentVms.map((vm) => (
          <button
            key={vm}
            id={`filter-${vm.toLowerCase()}`}
            style={chipStyle(filter === vm)}
            onClick={() => { setFilter(vm); setPage(0); }}
          >
            {getBadgeLabel(vm).text} Only
          </button>
        ))}

        <button
          id="filter-boundary"
          style={chipStyle(filter === 'boundary')}
          onClick={() => { setFilter('boundary'); setPage(0); }}
        >
          Boundaries / CPI ({report.vm_boundary_count})
        </button>

        <input
          id="trace-search"
          type="search"
          placeholder="Search opcode / instruction / HostIO…"
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
            width: 260,
          }}
        />
      </div>

      {/* Steps List */}
      <div className="trace-list" role="list">
        {visible.length === 0 ? (
          <div style={{ padding: 'var(--sp-4)', textAlign: 'center', color: 'var(--color-text-muted)', fontSize: 13 }}>
            No steps matching filter or search.
          </div>
        ) : (
          visible.map((step) => <StepRow key={step.index} step={step} report={report} />)
        )}
      </div>

      {/* Pagination */}
      {pageCount > 1 && (
        <div className="trace-pagination">
          <button
            id="trace-prev"
            disabled={page === 0}
            onClick={() => setPage((p) => Math.max(0, p - 1))}
            style={{
              padding: '4px 10px',
              background: 'var(--color-bg-raised)',
              border: '1px solid var(--color-border)',
              borderRadius: 4,
              color: 'var(--color-text-secondary)',
              cursor: page === 0 ? 'not-allowed' : 'pointer',
              opacity: page === 0 ? 0.4 : 1,
            }}
          >
            ← Prev
          </button>
          <span>
            Page {page + 1} of {pageCount} ({filtered.length} total)
          </span>
          <button
            id="trace-next"
            disabled={page >= pageCount - 1}
            onClick={() => setPage((p) => Math.min(pageCount - 1, p + 1))}
            style={{
              padding: '4px 10px',
              background: 'var(--color-bg-raised)',
              border: '1px solid var(--color-border)',
              borderRadius: 4,
              color: 'var(--color-text-secondary)',
              cursor: page >= pageCount - 1 ? 'not-allowed' : 'pointer',
              opacity: page >= pageCount - 1 ? 0.4 : 1,
            }}
          >
            Next →
          </button>
        </div>
      )}
    </div>
  );
}
