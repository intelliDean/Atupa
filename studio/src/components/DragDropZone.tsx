import React, { useState, useCallback } from 'react';
import type { StudioReport } from '../types/trace';

interface Props {
  onLoad: (report: StudioReport) => void;
}

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
          const data = JSON.parse(e.target?.result as string) as StudioReport;
          const isSingle = (data as any).tx_hash && Array.isArray((data as any).steps);
          const isDiffReport = (data as any).type === 'diff' && (data as any).base && (data as any).target;
          
          if (!isSingle && !isDiffReport) {
            setError('File does not appear to be an Atupa trace report or comparison.');
            return;
          }
          setError(null);
          onLoad(data);
        } catch {
          setError('Failed to parse JSON — is this a valid Atupa trace?');
        }
      };
      reader.readAsText(file);
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
    <div
      id="drop-zone"
      className={`drop-zone${dragging ? ' dragging' : ''}`}
      onDragOver={(e) => { e.preventDefault(); setDragging(true); }}
      onDragLeave={() => setDragging(false)}
      onDrop={onDrop}
    >
      <div className="drop-icon">🏮</div>

      <div>
        <div className="drop-title">Drop your Atupa trace here</div>
        <div className="drop-subtitle" style={{ marginTop: 8 }}>
          Generate a trace with the CLI, then drop the <code style={{ color: 'var(--color-amber)', fontSize: 12 }}>report.json</code> file
          to visualize its unified EVM + Stylus execution.
        </div>
      </div>

      <label htmlFor="file-input" style={{ cursor: 'pointer' }}>
        <span className="drop-cta" role="button" aria-label="Choose file">
          <span>📂</span> Choose File
        </span>
      </label>
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

      <div className="drop-hint">
        atupa capture --tx 0x... --rpc &lt;URL&gt; --output report.json
      </div>
    </div>
  );
}
