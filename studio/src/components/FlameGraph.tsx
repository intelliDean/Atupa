// ─── Atupa Studio — Interactive Flamegraph Component ─────────────────────────
//
// Renders a zoomable, searchable SVG flamegraph that visualises the unified
// EVM + Stylus call tree.  No external render deps — pure React + SVG.

import React, {
  useState,
  useMemo,
  useCallback,
  useRef,
  useEffect,
} from 'react';
import type { FlameNode } from '../types/reportToTree';

// ─── Constants & Tokens ───────────────────────────────────────────────────────

const BAR_H    = 24;   // px height per depth row
const TEXT_PAD = 6;    // horizontal padding inside a bar
const MIN_PX   = 2;    // skip bars narrower than this (avoids SVG clutter)
const APPROX_CHAR_W = 6.5; // JetBrains Mono ≈ 6.5px per char @ 11px

// Colors map to the existing Atupa design-system CSS variables
const COLORS = {
  evmFill:       '#1e2a45',
  evmStroke:     '#2e4a7a',
  evmText:       '#93c5fd',
  stylusFill:    '#2a1922',
  stylusStroke:  '#7f1d2e',
  stylusText:    '#ff8fa3',
  boundaryFill:  '#1e1833',
  boundaryStroke:'#6d28d9',
  boundaryText:  '#a78bfa',
  highlightFill: '#7f1d2e',
  rootFill:      '#0d0f1a',
  rootStroke:    '#1e2435',
  rootText:      '#64748b',
  starknetFill:   '#1e1b4b',
  starknetStroke: '#4338ca',
  starknetText:   '#a5b4fc',
  solanaFill:     '#064e3b',
  solanaStroke:   '#059669',
  solanaText:     '#6ee7b7',
  stellarFill:    '#172554',
  stellarStroke:  '#1e3a8a',
  stellarText:    '#93c5fd',
  tooltipBg:     '#181c2a',
  tooltipBorder: '#2e3a5a',
  tooltipText:   '#e2e8f0',
};

// ─── Tooltip ─────────────────────────────────────────────────────────────────

interface TooltipState {
  x: number;
  y: number;
  node: FlameNode;
}

function Tooltip({ tip, rootValue }: { tip: TooltipState, rootValue: number }) {
  const totalPct = ((tip.node.value / rootValue) * 100).toFixed(2);
  const selfPct  = ((tip.node.selfCost / rootValue) * 100).toFixed(2);

  const vmLabel = {
    Evm: 'EVM',
    Stylus: 'WASM/Stylus',
    Starknet: 'Starknet Cairo',
    Solana: 'Solana SVM',
    Stellar: 'Stellar Soroban',
  }[tip.node.vm] || tip.node.vm;

  const vmColor = {
    Evm: COLORS.evmText,
    Stylus: COLORS.stylusText,
    Starknet: COLORS.starknetText,
    Solana: COLORS.solanaText,
    Stellar: COLORS.stellarText,
  }[tip.node.vm] || '#fff';

  return (
    <foreignObject
      x={tip.x + 12}
      y={tip.y - 8}
      width={260}
      height={110}
      style={{ pointerEvents: 'none', overflow: 'visible' }}
    >
      <div
        style={{
          background: COLORS.tooltipBg,
          border: `1px solid ${COLORS.tooltipBorder}`,
          borderRadius: 8,
          padding: '8px 12px',
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: 11,
          color: COLORS.tooltipText,
          lineHeight: 1.6,
          boxShadow: '0 4px 24px rgba(0,0,0,0.6)',
          whiteSpace: 'nowrap',
        }}
      >
        <div style={{ fontWeight: 700, fontSize: 12, marginBottom: 4, color: '#fff' }}>
          {tip.node.name}
        </div>
        <div style={{ color: '#94a3b8' }}>
          VM: <span style={{ color: vmColor }}>{vmLabel}</span>
        </div>
        <div style={{ color: '#94a3b8' }}>
          Total: <span style={{ color: '#e2e8f0' }}>{tip.node.value.toLocaleString('en-US', { maximumFractionDigits: 2 })} gas ({totalPct}%)</span>
        </div>
        <div style={{ color: '#94a3b8' }}>
          Self:  <span style={{ color: '#e2e8f0' }}>{tip.node.selfCost.toLocaleString('en-US', { maximumFractionDigits: 2 })} gas ({selfPct}%)</span>
        </div>
        <div style={{ color: '#94a3b8' }}>
          Depth: <span style={{ color: '#e2e8f0' }}>{tip.node.depth}</span>
          {tip.node.is_vm_boundary && (
            <span style={{ color: '#a78bfa', marginLeft: 8 }}>⇌ Boundary</span>
          )}
        </div>
      </div>
    </foreignObject>
  );
}

// ─── Layout ──────────────────────────────────────────────────────────────────

interface LayoutNode {
  node: FlameNode;
  x: number;     // 0..1 fraction of total width
  w: number;     // 0..1 fraction of total width
  row: number;   // depth row (0 = top)
}

/**
 * Recursively flattens the tree into a list of {node, x, w, row} entries.
 * `x0` and `x1` are the fractional span [0..1] of the parent's domain.
 */
function layoutTree(
  node: FlameNode,
  x0: number,
  x1: number,
  row: number,
  result: LayoutNode[],
) {
  result.push({ node, x: x0, w: x1 - x0, row });

  if (node.children.length === 0) return;

  const totalChildValue = node.children.reduce((s, c) => s + c.value, 0);
  if (totalChildValue === 0) return;

  let cx = x0;
  for (const child of node.children) {
    const cw = ((x1 - x0) * child.value) / totalChildValue;
    layoutTree(child, cx, cx + cw, row + 1, result);
    cx += cw;
  }
}

// ─── Bar ─────────────────────────────────────────────────────────────────────

interface BarProps {
  lnode: LayoutNode;
  svgWidth: number;
  zoomX: number;  // current zoom origin (fraction)
  zoomW: number;  // current zoom width  (fraction)
  highlight: string;
  onHover: (tip: TooltipState | null, evt: React.MouseEvent) => void;
  onClick: (n: FlameNode) => void;
}

const Bar = React.memo(function Bar({
  lnode, svgWidth, zoomX, zoomW, highlight, onHover, onClick,
}: BarProps) {
  const { node, x, w, row } = lnode;

  // Map fraction → pixel within the visible zoom window
  const visX = ((x - zoomX) / zoomW) * svgWidth;
  const visW = (w / zoomW) * svgWidth;

  if (visW < MIN_PX) return null;

  const py = row * (BAR_H + 2);

  // Color logic
  let fill: string, stroke: string, textColor: string;
  if (row === 0) {
    fill = COLORS.rootFill; stroke = COLORS.rootStroke; textColor = COLORS.rootText;
  } else if (node.is_vm_boundary) {
    fill = COLORS.boundaryFill; stroke = COLORS.boundaryStroke; textColor = COLORS.boundaryText;
  } else if (node.vm === 'Stylus') {
    fill = COLORS.stylusFill; stroke = COLORS.stylusStroke; textColor = COLORS.stylusText;
  } else if (node.vm === 'Starknet') {
    fill = COLORS.starknetFill; stroke = COLORS.starknetStroke; textColor = COLORS.starknetText;
  } else if (node.vm === 'Solana') {
    fill = COLORS.solanaFill; stroke = COLORS.solanaStroke; textColor = COLORS.solanaText;
  } else if (node.vm === 'Stellar') {
    fill = COLORS.stellarFill; stroke = COLORS.stellarStroke; textColor = COLORS.stellarText;
  } else {
    fill = COLORS.evmFill; stroke = COLORS.evmStroke; textColor = COLORS.evmText;
  }

  const isHighlighted =
    highlight.length >= 2 &&
    node.name.toLowerCase().includes(highlight.toLowerCase());

  if (isHighlighted) {
    fill = COLORS.highlightFill;
    stroke = '#ff2a4a';
  }

  // Label — truncate to fit
  const maxChars = Math.max(0, Math.floor((visW - TEXT_PAD * 2) / APPROX_CHAR_W));
  let label = node.name;
  if (label.length > maxChars) {
    label = maxChars > 3 ? label.slice(0, maxChars - 1) + '…' : '';
  }

  return (
    <g
      style={{ cursor: row === 0 ? 'default' : 'pointer' }}
      onClick={() => row > 0 && onClick(node)}
      onMouseMove={(e) => onHover({ x: e.nativeEvent.offsetX, y: py, node }, e)}
      onMouseLeave={() => onHover(null, {} as React.MouseEvent)}
    >
      <rect
        x={visX + 1}
        y={py + 1}
        width={Math.max(0, visW - 2)}
        height={BAR_H - 2}
        rx={3}
        fill={fill}
        stroke={isHighlighted ? '#ff2a4a' : stroke}
        strokeWidth={isHighlighted ? 1.5 : 0.8}
        style={{ transition: 'fill 120ms ease' }}
      />
      {label && (
        <text
          x={visX + TEXT_PAD}
          y={py + BAR_H / 2 + 4}
          fill={textColor}
          fontSize={11}
          fontFamily="'JetBrains Mono', monospace"
          style={{ pointerEvents: 'none', userSelect: 'none' }}
        >
          {label}
        </text>
      )}
    </g>
  );
});

// ─── Breadcrumb ───────────────────────────────────────────────────────────────

function Breadcrumb({
  trail,
  onJump,
}: {
  trail: FlameNode[];
  onJump: (idx: number) => void;
}) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 4,
        flexWrap: 'wrap',
        fontFamily: "'JetBrains Mono', monospace",
        fontSize: 11,
        color: '#64748b',
        marginBottom: 8,
        minHeight: 20,
      }}
    >
      {trail.map((n, i) => (
        <React.Fragment key={n.id}>
          {i > 0 && <span style={{ color: '#334155' }}>›</span>}
          <button
            onClick={() => onJump(i)}
            style={{
              background: 'none',
              border: 'none',
              padding: '1px 4px',
              borderRadius: 4,
              cursor: i < trail.length - 1 ? 'pointer' : 'default',
              color: i === trail.length - 1 ? '#93c5fd' : '#64748b',
              fontFamily: 'inherit',
              fontSize: 11,
              textDecoration: i < trail.length - 1 ? 'underline' : 'none',
            }}
          >
            {n.name.length > 22 ? n.name.slice(0, 20) + '…' : n.name}
          </button>
        </React.Fragment>
      ))}
    </div>
  );
}

// ─── FlameGraph ───────────────────────────────────────────────────────────────

interface Props {
  root: FlameNode;
  search?: string;
}

export function FlameGraph({ root, search = '' }: Props) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [svgWidth, setSvgWidth] = useState(800);
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);

  // Zoom state: the zoomed-in node trail (first = virtual root)
  const [zoomTrail, setZoomTrail] = useState<FlameNode[]>([root]);
  const zoomedNode = zoomTrail[zoomTrail.length - 1];

  // Recalculate when root changes (new report loaded)
  useEffect(() => {
    setZoomTrail([root]);
  }, [root]);

  // Observe container width
  useEffect(() => {
    if (!svgRef.current) return;
    const ro = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width;
      if (width) setSvgWidth(width);
    });
    ro.observe(svgRef.current);
    return () => ro.disconnect();
  }, []);

  // Compute full layout (all nodes, fractions)
  const allNodes = useMemo<LayoutNode[]>(() => {
    const result: LayoutNode[] = [];
    layoutTree(root, 0, 1, 0, result);
    return result;
  }, [root]);

  // Determine visible zoom window from the currently zoomed node
  const { zoomX, zoomW } = useMemo(() => {
    const found = allNodes.find((l) => l.node.id === zoomedNode.id);
    if (!found) return { zoomX: 0, zoomW: 1 };
    return { zoomX: found.x, zoomW: found.w };
  }, [allNodes, zoomedNode]);

  // Filter to only nodes visible in current zoom (partially or fully)
  const visibleNodes = useMemo(
    () =>
      allNodes.filter((l) => {
        if (l.w === 0) return false;
        const visW = (l.w / zoomW) * svgWidth;
        return visW >= MIN_PX;
      }),
    [allNodes, zoomW, svgWidth],
  );

  // SVG height
  const maxRow = useMemo(
    () => Math.max(...visibleNodes.map((l) => l.row), 0),
    [visibleNodes],
  );
  const svgHeight = (maxRow + 1) * (BAR_H + 2) + 8;

  // Handlers
  const handleClick = useCallback(
    (node: FlameNode) => {
      setZoomTrail((t) => [...t, node]);
      setTooltip(null);
    },
    [],
  );

  const handleBreadcrumb = useCallback((idx: number) => {
    setZoomTrail((t) => t.slice(0, idx + 1));
    setTooltip(null);
  }, []);

  const handleHover = useCallback(
    (tip: TooltipState | null, _evt: React.MouseEvent) => {
      setTooltip(tip);
    },
    [],
  );

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 0 }}>
      <Breadcrumb trail={zoomTrail} onJump={handleBreadcrumb} />

      {/* Legend */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 16,
          marginBottom: 10,
          fontSize: 10,
          fontFamily: "'JetBrains Mono', monospace",
          color: '#64748b',
        }}
      >
        {[
          { color: COLORS.evmStroke, label: 'EVM' },
          { color: COLORS.stylusStroke, label: 'Stylus' },
          { color: COLORS.starknetStroke, label: 'Starknet' },
          { color: COLORS.solanaStroke, label: 'Solana' },
          { color: COLORS.stellarStroke, label: 'Stellar' },
          { color: '#6d28d9', label: 'VM Boundary' },
          { color: '#ff2a4a', label: 'Search match' },
        ].map(({ color, label }) => (
          <span key={label} style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
            <span
              style={{
                width: 10,
                height: 10,
                borderRadius: 2,
                background: color,
                display: 'inline-block',
              }}
            />
            {label}
          </span>
        ))}
        <span style={{ marginLeft: 'auto', color: '#475569' }}>
          {allNodes.length} nodes · click to zoom
        </span>
      </div>

      <div
        style={{
          border: '1px solid #1e2435',
          borderRadius: 8,
          overflow: 'hidden',
          background: '#07080d',
        }}
      >
        <svg
          ref={svgRef}
          width="100%"
          height={svgHeight}
          style={{ display: 'block' }}
        >
          {visibleNodes.map((l) => (
            <Bar
              key={l.node.id}
              lnode={l}
              svgWidth={svgWidth}
              zoomX={zoomX}
              zoomW={zoomW}
              highlight={search}
              onHover={handleHover}
              onClick={handleClick}
            />
          ))}

          {tooltip && <Tooltip tip={tooltip} rootValue={root.value} />}
        </svg>
      </div>

      {/* Reset zoom hint */}
      {zoomTrail.length > 1 && (
        <div
          style={{
            marginTop: 8,
            fontSize: 11,
            color: '#475569',
            fontFamily: "'JetBrains Mono', monospace",
            textAlign: 'right',
          }}
        >
          <button
            id="flame-reset-zoom"
            onClick={() => setZoomTrail([root])}
            style={{
              background: 'none',
              border: '1px solid #1e2435',
              borderRadius: 4,
              color: '#64748b',
              padding: '2px 8px',
              fontSize: 11,
              cursor: 'pointer',
              fontFamily: 'inherit',
            }}
          >
            ↩ Reset zoom
          </button>
        </div>
      )}
    </div>
  );
}
