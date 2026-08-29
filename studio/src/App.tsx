import { useState, useCallback, useMemo, useEffect } from 'react';
import './styles/design-system.css';
import {
  aggregateHostIOs,
  fmtGas,
  fmtEquiv,
  shortHash,
  evmSteps,
  stylusSteps,
  solanaSteps,
  starknetSteps,
  stellarSteps,
  detectPrimaryVm,
  getRuntimeBadge,
  isDiff,
} from './types/trace';
import type { StudioReport } from './types/trace';
import { reportToTree } from './types/reportToTree';
import { DragDropZone } from './components/DragDropZone';
import { MetricCard } from './components/MetricCard';
import { HostIoAggregator } from './components/HostIoAggregator';
import { TraceInspector } from './components/TraceInspector';
import { FlameGraph } from './components/FlameGraph';
import { CategoryBreakdown } from './components/CategoryBreakdown';
import { DiffOverview } from './components/DiffOverview';

type View = 'overview' | 'flame' | 'trace' | 'hostio';

export default function App() {
  const [report, setReport] = useState<StudioReport | null>(null);
  const [view, setView] = useState<View>('overview');
  const [flameSearch, setFlameSearch] = useState('');

  const handleLoad = useCallback((r: StudioReport) => {
    setReport(r);
    setView('overview');
  }, []);

  // ── Auto-load from URL ─────────────────────────────────────────────────────
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get('auto') === 'true') {
      fetch('/auto-load.json')
        .then(res => {
          if (!res.ok) throw new Error('Report not found');
          return res.json();
        })
        .then(handleLoad)
        .catch(err => {
          console.warn('Auto-load failed or no report found:', err);
        });
    }
  }, [handleLoad]);

  const handleReset = useCallback(() => {
    setReport(null);
    setView('overview');
    setFlameSearch('');
  }, []);

  const activeTarget = report ? (isDiff(report) ? report.target : report) : null;
  const primaryVm = activeTarget ? detectPrimaryVm(activeTarget) : 'evm';
  const runtimeBadge = getRuntimeBadge(primaryVm);
  const hostIOs = activeTarget ? aggregateHostIOs(activeTarget) : [];
  
  const flameRoot = useMemo(
    () => (activeTarget ? reportToTree(activeTarget) : null),
    [activeTarget],
  );

  return (
    <div className="app-shell">
      {/* ── Top Bar ──────────────────────────────────────────────────────── */}
      <header className="app-topbar">
        <a className="brand" href="#" onClick={handleReset} aria-label="Atupa Studio home">
          <span className="brand-icon">🏮</span>
          <span className="brand-name">Atupa</span>
          <span className="brand-tag">Studio</span>
        </a>

        {report && (
          <>
            <span
              className="live-badge"
              style={{
                background: `color-mix(in srgb, ${runtimeBadge.color} 15%, transparent)`,
                borderColor: runtimeBadge.color,
                color: runtimeBadge.color,
              }}
            >
              <span className="live-dot" style={{ backgroundColor: runtimeBadge.color }} />
              {runtimeBadge.icon} {runtimeBadge.label}
            </span>

            {isDiff(report) && (
              <span className="live-badge" style={{ marginLeft: 6 }}>
                ⚖️ Diff Mode
              </span>
            )}

            <span
              className="topbar-tx"
              title={isDiff(report) ? `${report.base.tx_hash} vs ${report.target.tx_hash}` : report.tx_hash}
            >
              {isDiff(report) ? 'Execution Comparison' : report.tx_hash}
            </span>

            <button
              id="btn-reset"
              onClick={handleReset}
              style={{
                marginLeft: 8,
                background: 'none',
                border: '1px solid var(--color-border)',
                borderRadius: 6,
                color: 'var(--color-text-muted)',
                padding: '4px 12px',
                fontSize: 11,
                cursor: 'pointer',
                fontFamily: 'var(--font-ui)',
              }}
            >
              ✕ Clear
            </button>
          </>
        )}
      </header>

      {/* ── Sidebar ──────────────────────────────────────────────────────── */}
      <nav className="app-sidebar" aria-label="Main navigation">
        <div className="sidebar-section-label">Views</div>

        {([
          { id: 'overview' as const, icon: '📊', label: 'Overview' },
          { id: 'flame' as const,    icon: '🔆', label: 'Visual Trace' },
          { id: 'trace' as const,    icon: '🧩', label: 'Trace Inspector' },
          ...(hostIOs.length > 0 ? [{ id: 'hostio' as const, icon: '🔥', label: 'HostIO Hot Paths' }] : []),
        ]).map((v) => (
          <button
            key={v.id}
            id={`nav-${v.id}`}
            className={`sidebar-nav-item${view === v.id && report ? ' active' : ''}`}
            onClick={() => report && setView(v.id)}
            disabled={!report}
            style={{ opacity: report ? 1 : 0.4 }}
          >
            <span className="nav-icon">{v.icon}</span>
            {v.label}
          </button>
        ))}

        {report && !isDiff(report) && (
          <div className="sidebar-meta">
            <div>tx: {shortHash(report.tx_hash)}</div>
            <div>total steps: {report.steps.length.toLocaleString()}</div>
            {primaryVm === 'solana' && <div>svm: {solanaSteps(report).length.toLocaleString()}</div>}
            {primaryVm === 'starknet' && <div>cairo: {starknetSteps(report).length.toLocaleString()}</div>}
            {primaryVm === 'stellar' && <div>soroban: {stellarSteps(report).length.toLocaleString()}</div>}
            {primaryVm === 'stylus' && (
              <>
                <div>evm: {evmSteps(report).length.toLocaleString()}</div>
                <div>wasm: {stylusSteps(report).length.toLocaleString()}</div>
              </>
            )}
            {primaryVm === 'evm' && <div>evm: {evmSteps(report).length.toLocaleString()}</div>}
          </div>
        )}

        {report && isDiff(report) && (
          <div className="sidebar-meta">
            <div style={{ color: 'var(--color-text-primary)', fontWeight: 'bold' }}>⚖️ DELTA</div>
            <div style={{ color: report.metrics.gas_delta > 0 ? '#ff4d4d' : '#4dff88' }}>
              Gas: {report.metrics.gas_delta > 0 ? '+' : ''}{fmtGas(report.metrics.gas_delta)} ({report.metrics.gas_pct > 0 ? '+' : ''}{report.metrics.gas_pct.toFixed(1)}%)
            </div>
          </div>
        )}
      </nav>

      {/* ── Main Content ─────────────────────────────────────────────────── */}
      <main className="app-main">
        {!report ? (
          <DragDropZone onLoad={handleLoad} />
        ) : (
          <>
            {view === 'overview' && (
              <>
                {isDiff(report) ? (
                  <DiffOverview report={report} />
                ) : (
                  <>
                    {/* Section: Cost Breakdown */}
                    <div className="glass-card">
                      <div className="section-header">
                        <span className="section-title">Cost Breakdown by Category</span>
                        <div className="section-divider" />
                      </div>
                      <CategoryBreakdown report={report} />
                    </div>
                  </>
                )}

                {/* Section: Dynamic Multi-VM Metrics */}
                {activeTarget && (
                  <div className="glass-card">
                    <div className="section-header">
                      <span className="section-title">
                        {runtimeBadge.icon} {runtimeBadge.label} Execution Metrics
                      </span>
                      <div className="section-divider" />
                    </div>
                    <div className="metric-grid">
                      {primaryVm === 'solana' && (
                        <>
                          <MetricCard
                            kind="stylus"
                            icon="☀️"
                            label="Solana Compute Units"
                            value={fmtGas(activeTarget.total_unified_cost)}
                            sub="consumed CU"
                          />
                          <MetricCard
                            kind="steps"
                            icon="🧩"
                            label="Instruction Steps"
                            value={fmtGas(activeTarget.steps.length)}
                            sub="SVM instructions"
                          />
                          <MetricCard
                            kind="evm"
                            icon="💾"
                            label="State & Token Writes"
                            value={fmtGas(activeTarget.category_costs.StorageWrite || 0)}
                            sub="SPL balance & account updates"
                          />
                          <MetricCard
                            kind="stylus"
                            icon="🔐"
                            label="Crypto & Invariants"
                            value={fmtGas(activeTarget.category_costs.Crypto || 0)}
                            sub="AMM math & hashing"
                          />
                          <MetricCard
                            kind="boundary"
                            icon="⇌"
                            label="Cross-Program CPIs"
                            value={fmtGas(activeTarget.vm_boundary_count)}
                            sub="nested program calls"
                          />
                        </>
                      )}

                      {primaryVm === 'starknet' && (
                        <>
                          <MetricCard
                            kind="stylus"
                            icon="🐺"
                            label="Cairo Execution Cost"
                            value={fmtGas(activeTarget.total_unified_cost)}
                            sub="Cairo steps + builtins"
                          />
                          <MetricCard
                            kind="steps"
                            icon="🧩"
                            label="Function Invocations"
                            value={fmtGas(activeTarget.steps.length)}
                            sub="Cairo call frames"
                          />
                          <MetricCard
                            kind="evm"
                            icon="🔐"
                            label="Crypto Builtins"
                            value={fmtGas(activeTarget.category_costs.Crypto || 0)}
                            sub="ECDSA & Pedersen"
                          />
                          <MetricCard
                            kind="stylus"
                            icon="⚙️"
                            label="Core Validation Steps"
                            value={fmtGas(activeTarget.category_costs.Execution || 0)}
                            sub="Range Check & VM steps"
                          />
                          <MetricCard
                            kind="boundary"
                            icon="⇌"
                            label="Contract Crossings"
                            value={fmtGas(activeTarget.vm_boundary_count)}
                            sub="cross-contract invocations"
                          />
                        </>
                      )}

                      {primaryVm === 'stellar' && (
                        <>
                          <MetricCard
                            kind="stylus"
                            icon="🚀"
                            label="Soroban Resource Cost"
                            value={fmtGas(activeTarget.total_unified_cost)}
                            sub="CPU & Memory units"
                          />
                          <MetricCard
                            kind="steps"
                            icon="🧩"
                            label="Diagnostic Events"
                            value={fmtGas(activeTarget.steps.length)}
                            sub="Soroban events"
                          />
                          <MetricCard
                            kind="evm"
                            icon="💾"
                            label="State Ledger Updates"
                            value={fmtGas(activeTarget.category_costs.StorageWrite || 0)}
                            sub="put_contract_data"
                          />
                          <MetricCard
                            kind="stylus"
                            icon="🔐"
                            label="Crypto Hashing"
                            value={fmtGas(activeTarget.category_costs.Crypto || 0)}
                            sub="SHA256 & verification"
                          />
                          <MetricCard
                            kind="boundary"
                            icon="⇌"
                            label="HostFn Calls"
                            value={fmtGas(activeTarget.vm_boundary_count)}
                            sub="Host Function invocations"
                          />
                        </>
                      )}

                      {primaryVm === 'stylus' && (
                        <>
                          <MetricCard
                            kind="evm"
                            icon="⛽"
                            label="EVM Trace Gas"
                            value={fmtGas(activeTarget.total_evm_gas)}
                            sub="gas units"
                          />
                          <MetricCard
                            kind="stylus"
                            icon="🦾"
                            label="Stylus Ink"
                            value={fmtGas(activeTarget.total_stylus_ink)}
                            sub={`≈ ${fmtEquiv(activeTarget.total_stylus_gas_equiv)} gas-equiv`}
                          />
                          <MetricCard
                            kind="steps"
                            icon="🧩"
                            label="EVM Steps"
                            value={fmtGas(evmSteps(activeTarget).length)}
                            sub="struct log entries"
                          />
                          <MetricCard
                            kind="stylus"
                            icon="📡"
                            label="Stylus HostIOs"
                            value={fmtGas(stylusSteps(activeTarget).length)}
                            sub="WASM host calls"
                          />
                          <MetricCard
                            kind="boundary"
                            icon="⇌"
                            label="VM Boundaries"
                            value={fmtGas(activeTarget.vm_boundary_count)}
                            sub="EVM ↔ WASM crossings"
                          />
                        </>
                      )}

                      {primaryVm === 'evm' && (
                        <>
                          <MetricCard
                            kind="evm"
                            icon="⛽"
                            label="On-Chain EVM Gas"
                            value={fmtGas(activeTarget.total_evm_gas)}
                            sub="gas units"
                          />
                          <MetricCard
                            kind="steps"
                            icon="🧩"
                            label="Opcode Steps"
                            value={fmtGas(activeTarget.steps.length)}
                            sub="struct log entries"
                          />
                          <MetricCard
                            kind="evm"
                            icon="💾"
                            label="Storage Writes"
                            value={fmtGas(activeTarget.category_costs.StorageWrite || 0)}
                            sub="SSTORE operations"
                          />
                          <MetricCard
                            kind="stylus"
                            icon="📖"
                            label="Storage Reads"
                            value={fmtGas(activeTarget.category_costs.StorageRead || 0)}
                            sub="SLOAD operations"
                          />
                          <MetricCard
                            kind="boundary"
                            icon="📡"
                            label="External Calls"
                            value={fmtGas(activeTarget.category_costs.Call || 0)}
                            sub="CALL / STATICCALL"
                          />
                        </>
                      )}
                    </div>
                  </div>
                )}

                {/* Section: HostIO summary on overview */}
                {hostIOs.length > 0 && (
                  <div className="glass-card">
                    <div className="section-header">
                      <span className="section-title">🔥 Top Ink Consumers</span>
                      <div className="section-divider" />
                    </div>
                    <HostIoAggregator rows={hostIOs.slice(0, 6)} />
                  </div>
                )}
              </>
            )}

            {view === 'flame' && flameRoot && (
              <div className="glass-card">
                <div className="section-header">
                  <span className="section-title">🔆 Visual Trace</span>
                  <div className="section-divider" />
                  <input
                    id="flame-search"
                    type="search"
                    placeholder="Search node…"
                    value={flameSearch}
                    onChange={(e) => setFlameSearch(e.target.value)}
                    style={{
                      padding: '4px 10px',
                      background: 'var(--color-bg-raised)',
                      border: '1px solid var(--color-border)',
                      borderRadius: 6,
                      color: 'var(--color-text-primary)',
                      fontSize: 12,
                    }}
                  />
                </div>
                <FlameGraph root={flameRoot} search={flameSearch} />
              </div>
            )}

            {view === 'trace' && activeTarget && (
              <div className="glass-card">
                <div className="section-header">
                  <span className="section-title">🧩 Trace Inspector</span>
                  <div className="section-divider" />
                </div>
                <TraceInspector report={activeTarget} />
              </div>
            )}

            {view === 'hostio' && hostIOs.length > 0 && (
              <div className="glass-card">
                <div className="section-header">
                  <span className="section-title">🔥 Stylus HostIO Hot Paths</span>
                  <div className="section-divider" />
                </div>
                <HostIoAggregator rows={hostIOs} />
              </div>
            )}
          </>
        )}
      </main>
    </div>
  );
}
