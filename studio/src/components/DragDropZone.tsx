import React, { useState, useCallback } from 'react';
import type { StudioReport } from '../types/trace';

interface Props {
  onLoad: (report: StudioReport) => void;
}

const DEMO_PRESETS = [
  { id: 'stylus', name: 'Arbitrum Stylus (Dual-VM)', icon: '🌐', path: '/demos/stylus.json', desc: 'EVM + Stylus WASM HostIO execution' },
  { id: 'solana', name: 'Solana (SVM)', icon: '☀️', path: '/demos/solana.json', desc: 'Raydium Swap Compute Units & SPL transfers' },
  { id: 'starknet', name: 'Starknet (Cairo)', icon: '🐺', path: '/demos/starknet.json', desc: 'Cairo execution & ECDSA/Pedersen builtins' },
  { id: 'stellar', name: 'Stellar (Soroban)', icon: '🚀', path: '/demos/stellar.json', desc: 'Diagnostic events & Soroban HostFn weights' },
  { id: 'aave', name: 'Aave v3 / GHO Audit', icon: '👻', path: '/demos/aave.json', desc: 'Supply, flash loans & liquidation state' },
  { id: 'lido', name: 'Lido stETH Audit', icon: '💧', path: '/demos/lido.json', desc: 'Staking pipeline, rebase oracle & withdrawals' },
  { id: 'diff', name: 'Differential Diff (Uniswap v3 vs v4)', icon: '⚖️', path: '/demos/diff.json', desc: 'Gas delta & transient storage regression check' },
];

export function DragDropZone({ onLoad }: Props) {
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const parseFile = useCallback(
    (file: File) => {
      if (!file.name.endsWith('.json')) {
        setError('Please drop a valid Atupa JSON trace file.');
        return;
      }
      const reader = new FileReader();
      reader.onload = (e) => {
        try {
          const data = JSON.parse(e.target?.result as string) as Record<string, unknown>;
          const isSingle = typeof data.tx_hash === 'string' && Array.isArray(data.steps);
          const isDiffReport = data.type === 'diff' && typeof data.base === 'object' && typeof data.target === 'object';
          
          if (!isSingle && !isDiffReport) {
            setError('File does not appear to be an Atupa trace report or comparison.');
            return;
          }
          setError(null);
          onLoad(data as unknown as StudioReport);
        } catch {
          setError('Failed to parse JSON — is this a valid Atupa trace?');
        }
      };
      reader.readAsText(file);
    },
    [onLoad]
  );

  const loadPreset = useCallback(
    (path: string) => {
      setError(null);
      fetch(path)
        .then((res) => {
          if (!res.ok) throw new Error('Preset not found');
          return res.json();
        })
        .then((data) => onLoad(data as StudioReport))
        .catch(() => setError('Failed to load demo preset.'));
    },
    [onLoad]
  );

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragging(false);
      const file = e.dataTransfer.files[0];
      if (file) parseFile(file);
    },
    [parseFile]
  );

  const onFileInput = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (file) parseFile(file);
    },
    [parseFile]
  );

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 24, maxWidth: 900, margin: '0 auto', width: '100%' }}>
      <div
        id="drop-zone"
        className={`drop-zone${dragging ? ' dragging' : ''}`}
        onDragOver={(e) => { e.preventDefault(); setDragging(true); }}
        onDragLeave={() => setDragging(false)}
        onDrop={onDrop}
      >
        <div className="drop-icon">🏮</div>

        <div>
          <div className="drop-title">Universal Multi-VM Trace Visualizer</div>
          <div className="drop-subtitle" style={{ marginTop: 8 }}>
            Drop any <code style={{ color: 'var(--color-amber)', fontSize: 12 }}>report.json</code> from EVM, Arbitrum Stylus, Solana, Starknet, or Stellar.
          </div>
        </div>

        <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
          <label htmlFor="file-input" style={{ cursor: 'pointer' }}>
            <span className="drop-cta" role="button" aria-label="Choose file">
              <span>📂</span> Upload Local Report
            </span>
          </label>
        </div>

        <input
          id="file-input"
          type="file"
          accept=".json"
          onChange={onFileInput}
          style={{ display: 'none' }}
        />

        {error && (
          <div style={{
            color: 'var(--color-crimson)',
            fontSize: 12,
            background: 'var(--color-crimson-glow)',
            padding: '8px 14px',
            borderRadius: 6,
            border: '1px solid var(--color-border-accent)',
          }}>
            ⚠ {error}
          </div>
        )}
      </div>

      {/* ── Multi-VM & Protocol Presets ────────────────────────────────────────── */}
      <div className="glass-card" style={{ padding: '20px 24px' }}>
        <div className="section-header" style={{ marginBottom: 16 }}>
          <span className="section-title">✨ Explore Preloaded Multi-VM & Protocol Traces</span>
          <div className="section-divider" />
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))', gap: 12 }}>
          {DEMO_PRESETS.map((preset) => (
            <button
              key={preset.id}
              id={`btn-preset-${preset.id}`}
              onClick={() => loadPreset(preset.path)}
              style={{
                display: 'flex',
                alignItems: 'flex-start',
                gap: 12,
                padding: '12px 14px',
                background: 'var(--color-bg-raised)',
                border: '1px solid var(--color-border)',
                borderRadius: 8,
                color: 'var(--color-text-primary)',
                textAlign: 'left',
                cursor: 'pointer',
                transition: 'all 150ms ease',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.borderColor = 'var(--color-border-accent)';
                e.currentTarget.style.transform = 'translateY(-2px)';
                e.currentTarget.style.boxShadow = '0 4px 12px rgba(255, 42, 74, 0.15)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.borderColor = 'var(--color-border)';
                e.currentTarget.style.transform = 'translateY(0)';
                e.currentTarget.style.boxShadow = 'none';
              }}
            >
              <span style={{ fontSize: 22, lineHeight: 1 }}>{preset.icon}</span>
              <div style={{ flex: 1 }}>
                <div style={{ fontWeight: 600, fontSize: 13, color: 'var(--color-text-primary)' }}>{preset.name}</div>
                <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 2 }}>{preset.desc}</div>
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
