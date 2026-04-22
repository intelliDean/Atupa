import { useState, useCallback, useMemo, useEffect } from 'react';
import './styles/design-system.css';
import type { StitchedReport } from './types/trace';
import {
  aggregateHostIOs,
  fmtGas,
  fmtEquiv,
  shortHash,
  evmSteps,
  stylusSteps,
} from './types/trace';
import { reportToTree } from './types/reportToTree';
import { DragDropZone } from './components/DragDropZone';
import { MetricCard } from './components/MetricCard';
import { HostIoAggregator } from './components/HostIoAggregator';
import { TraceInspector } from './components/TraceInspector';
import { FlameGraph } from './components/FlameGraph';
import { CategoryBreakdown } from './components/CategoryBreakdown';

type View = 'overview' | 'flame' | 'trace' | 'hostio';

export default function App() {
  const [report, setReport] = useState<StitchedReport | null>(null);
  const [view, setView] = useState<View>('overview');
  const [flameSearch, setFlameSearch] = useState('');

  const handleLoad = useCallback((r: StitchedReport) => {
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

  const hostIOs = report ? aggregateHostIOs(report) : [];
  const flameRoot = useMemo(
    () => (report ? reportToTree(report) : null),
    [report],
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
            <span className="live-badge">
              <span className="live-dot" />
              Loaded
            </span>
            <span className="topbar-tx" title={report.tx_hash}>
              {report.tx_hash}
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

        {(['overview', 'flame', 'trace', 'hostio'] as View[]).map((v) => {
          const meta = {
            overview: { icon: '📊', label: 'Overview' },
            flame:    { icon: '🔆', label: 'Visual Trace' },
            trace:    { icon: '🧩', label: 'Trace Inspector' },
            hostio:   { icon: '🔥', label: 'HostIO Hot Paths' },
          }[v];
          return (
            <button
              key={v}
              id={`nav-${v}`}
              className={`sidebar-nav-item${view === v && report ? ' active' : ''}`}
              onClick={() => report && setView(v)}
              disabled={!report}
              style={{ opacity: report ? 1 : 0.4 }}
            >
              <span className="nav-icon">{meta.icon}</span>
              {meta.label}
            </button>
          );
        })}

        {report && (
          <div className="sidebar-meta">
            <div>tx: {shortHash(report.tx_hash)}</div>
            <div>steps: {report.steps.length.toLocaleString()}</div>
            <div>evm: {evmSteps(report).length.toLocaleString()}</div>
            <div>wasm: {stylusSteps(report).length.toLocaleString()}</div>
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
                {/* Section: Cost Breakdown */}
                <div className="glass-card">
                  <div className="section-header">
                    <span className="section-title">Cost Breakdown by Category</span>
                    <div className="section-divider" />
                  </div>
                  <CategoryBreakdown report={report} />
                </div>

                {/* Section: Metrics */}
                <div className="glass-card">
                  <div className="section-header">
                    <span className="section-title">Execution Metrics</span>
                    <div className="section-divider" />
                  </div>
                  <div className="metric-grid">
                    <MetricCard
                      kind="evm"
                      icon="⛽"
                      label="EVM Trace Gas"
                      value={fmtGas(report.total_evm_gas)}
                      sub="gas units"
                    />
                    <MetricCard
                      kind="stylus"
                      icon="🦾"
                      label="Stylus Ink"
                      value={fmtGas(report.total_stylus_ink)}
                      sub={`≈ ${fmtEquiv(report.total_stylus_gas_equiv)} gas-equiv`}
                    />
                    <MetricCard
                      kind="steps"
                      icon="🧩"
                      label="EVM Steps"
                      value={fmtGas(evmSteps(report).length)}
                      sub="struct log entries"
                    />
                    <MetricCard
                      kind="stylus"
                      icon="📡"
                      label="Stylus HostIOs"
                      value={fmtGas(stylusSteps(report).length)}
                      sub="WASM host calls"
                    />
                    <MetricCard
                      kind="boundary"
                      icon="⇌"
                      label="VM Boundaries"
                      value={fmtGas(report.vm_boundary_count)}
                      sub="EVM → WASM crossings"
                    />
                  </div>
                </div>

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
                      fontSize: 11,
                      fontFamily: 'var(--font-mono)',
                      outline: 'none',
                      width: 180,
                    }}
                  />
                </div>
                <FlameGraph root={flameRoot} search={flameSearch} />
              </div>
            )}

            {view === 'hostio' && (
              <div className="glass-card">
                <div className="section-header">
                  <span className="section-title">🔥 HostIO Hot Paths</span>
                  <div className="section-divider" />
                  <span style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)' }}>
                    {hostIOs.length} unique operations
                  </span>
                </div>
                <HostIoAggregator rows={hostIOs} />
              </div>
            )}

            {view === 'trace' && (
              <div className="glass-card">
                <div className="section-header">
                  <span className="section-title">🧩 Unified Execution Trace</span>
                  <div className="section-divider" />
                  <span style={{ fontSize: 11, color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)' }}>
                    {report.steps.length.toLocaleString()} total steps
                  </span>
                </div>
                <TraceInspector report={report} />
              </div>
            )}
          </>
        )}
      </main>
    </div>
  );
}
