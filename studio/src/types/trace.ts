// ─── Atupa Studio — Trace Data Types ─────────────────────────────────────────
// Mirrors the Rust `StitchedReport` / `UnifiedStep` structures.

export type VmKind = 'Evm' | 'Stylus';

export type GasCategory = 
  | 'StorageWrite' 
  | 'StorageRead' 
  | 'Memory' 
  | 'Crypto' 
  | 'Call' 
  | 'Execution' 
  | 'Precompile' 
  | 'Root' 
  | 'Other';

export interface UnifiedStep {
  index: number;
  vm: VmKind;
  label: string;
  gas_cost: number;
  cost_equiv: number;
  depth: number;
  is_vm_boundary: boolean;
  category: GasCategory;
  target_address?: string;
}

export interface StitchedReport {
  tx_hash: string;
  steps: UnifiedStep[];
  total_evm_gas: number;
  total_stylus_ink: number;
  total_stylus_gas_equiv: number;
  total_unified_cost: number;
  vm_boundary_count: number;
  category_costs: Record<GasCategory, number>;
  resolved_names: Record<string, string>;
}

export function getDisplayLabel(step: UnifiedStep, report: StitchedReport): string {
  if (step.target_address && report.resolved_names[step.target_address]) {
    return `${step.label} → ${report.resolved_names[step.target_address]}`;
  }
  return step.label;
}

export interface CategoryMeta {
  label: string;
  color: string;
  icon: string;
}

export const CATEGORY_META: Record<GasCategory, CategoryMeta> = {
  StorageWrite: { label: 'Storage Write', color: '#ff2a4a', icon: '💾' },
  StorageRead:  { label: 'Storage Read',  color: '#ff8c40', icon: '📖' },
  Memory:       { label: 'Memory Ops',    color: '#a78bfa', icon: '🧠' },
  Crypto:       { label: 'Crypto/Hashing',color: '#60d9ff', icon: '🔐' },
  Call:         { label: 'External Calls',color: '#2fe4c4', icon: '📡' },
  Execution:    { label: 'Core Execution',color: '#ffb340', icon: '⚙️' },
  Precompile:   { label: 'Precompiles',   color: '#9a9db5', icon: '⚡' },
  Root:         { label: 'Root Frame',    color: '#f0f0f8', icon: '🏁' },
  Other:        { label: 'Other',         color: '#555870', icon: '❓' },
};

// ─── Derived helpers ──────────────────────────────────────────────────────────

export interface AggregatedHostIO {
  name: string;
  total_cost_equiv: number;
  total_ink: number;
  call_count: number;
  pct: number;
}

export function evmSteps(report: StitchedReport): UnifiedStep[] {
  return report.steps.filter((s) => s.vm === 'Evm');
}

export function stylusSteps(report: StitchedReport): UnifiedStep[] {
  return report.steps.filter((s) => s.vm === 'Stylus');
}

export function aggregateHostIOs(report: StitchedReport): AggregatedHostIO[] {
  const map = new Map<string, { cost: number; ink: number; count: number }>();
  for (const step of report.steps) {
    if (step.vm !== 'Stylus') continue;
    const entry = map.get(step.label) ?? { cost: 0, ink: 0, count: 0 };
    entry.cost += step.cost_equiv;
    entry.ink += step.cost_equiv * 10_000; // approximate reverse
    entry.count += 1;
    map.set(step.label, entry);
  }

  const total = report.total_stylus_gas_equiv || 1;
  const rows: AggregatedHostIO[] = [];
  for (const [name, v] of map.entries()) {
    rows.push({
      name,
      total_cost_equiv: v.cost,
      total_ink: Math.round(v.ink),
      call_count: v.count,
      pct: (v.cost / total) * 100,
    });
  }
  return rows.sort((a, b) => b.total_cost_equiv - a.total_cost_equiv);
}

export function fmtGas(n: number): string {
  return n.toLocaleString('en-US');
}

export function fmtInk(n: number): string {
  return n.toLocaleString('en-US');
}

export function fmtEquiv(n: number): string {
  return n.toFixed(2);
}

export function shortHash(hash: string): string {
  if (hash.length < 12) return hash;
  return `${hash.slice(0, 8)}…${hash.slice(-6)}`;
}
