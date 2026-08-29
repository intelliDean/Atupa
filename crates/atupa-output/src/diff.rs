//! Visual differential flamegraph generation between baseline and target execution traces.

use atupa_core::{CollapsedStack, VmKind};
use std::collections::HashMap;

use crate::common::{
    BAR_GAP, BAR_HEIGHT, CHART_WIDTH, DIFF_HEADER_HEIGHT, MIN_BAR_PX, PADDING_LEFT,
    SEPARATOR_HEIGHT, SVG_WIDTH, render_empty_svg, stack_leaf, truncate_label,
};

/// Internal merged entry representing a unique stack path across baseline and target.
struct DiffEntry {
    stack: String,
    depth: u16,
    vm_kind: VmKind,
    baseline_weight: u64,
    target_weight: u64,
    resolved_label: Option<String>,
    target_address: Option<String>,
    reverted: bool,
}

/// Generates a visual differential flamegraph comparing a baseline trace against a target trace.
///
/// Green bars represent performance improvements (reduced gas / removed paths),
/// red bars represent regressions (increased gas / new paths), and gray bars represent stable paths.
pub fn generate_diff_flamegraph(
    baseline_stacks: &[CollapsedStack],
    target_stacks: &[CollapsedStack],
) -> anyhow::Result<String> {
    let merged = merge_diff_entries(baseline_stacks, target_stacks);
    let entries: Vec<&DiffEntry> = merged.values().collect();

    if entries.is_empty()
        || entries
            .iter()
            .all(|e| e.baseline_weight == 0 && e.target_weight == 0)
    {
        return Ok(render_empty_svg("No execution data found for diff."));
    }

    let evm_entries: Vec<&&DiffEntry> = entries
        .iter()
        .filter(|e| e.vm_kind != VmKind::Stylus)
        .collect();
    let mut wasm_entries: Vec<&&DiffEntry> = entries
        .iter()
        .filter(|e| e.vm_kind == VmKind::Stylus)
        .collect();
    let has_wasm = !wasm_entries.is_empty();

    let mut depths: Vec<u16> = evm_entries.iter().map(|e| e.depth).collect();
    depths.sort_unstable();
    depths.dedup();

    let mut body = String::new();
    let mut current_y = DIFF_HEADER_HEIGHT;

    // ── EVM / Standard Depth Lanes ────────────────────────────────────────────
    for depth in &depths {
        let mut lane_entries: Vec<&&&DiffEntry> =
            evm_entries.iter().filter(|e| e.depth == *depth).collect();
        lane_entries.sort_by(|a, b| a.stack.cmp(&b.stack));

        let lane_weight: u64 = lane_entries
            .iter()
            .map(|e| std::cmp::max(e.baseline_weight, e.target_weight))
            .sum();
        if lane_weight == 0 {
            continue;
        }

        let mut bar_x = PADDING_LEFT;
        for entry in &lane_entries {
            let node_weight = std::cmp::max(entry.baseline_weight, entry.target_weight);
            if node_weight == 0 {
                continue;
            }
            let bar_w = (node_weight as f64 / lane_weight as f64) * CHART_WIDTH;
            if bar_w < MIN_BAR_PX {
                continue;
            }

            render_diff_bar(&mut body, entry, bar_x, current_y, bar_w - 1.0, BAR_HEIGHT);
            bar_x += bar_w;
        }
        current_y += BAR_HEIGHT + BAR_GAP;
    }

    // ── Stylus WASM Lanes ─────────────────────────────────────────────────────
    if has_wasm {
        current_y += SEPARATOR_HEIGHT;

        // Draw separator
        body.push_str(&format!(
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#334155" stroke-width="1" stroke-dasharray="4"/>"##,
            PADDING_LEFT, current_y - 14.0, SVG_WIDTH - PADDING_LEFT, current_y - 14.0
        ));
        body.push_str(&format!(
            r##"<text x="{}" y="{}" font-size="11" fill="#64748b" font-family="Inter, monospace" text-anchor="middle" font-weight="bold">STYLUS HOST I/O</text>"##,
            SVG_WIDTH / 2.0, current_y - 10.0
        ));

        let global_wasm_weight: u64 = wasm_entries
            .iter()
            .map(|e| std::cmp::max(e.baseline_weight, e.target_weight))
            .sum();
        wasm_entries.sort_by(|a, b| a.stack.cmp(&b.stack));

        let mut bar_x = PADDING_LEFT;
        for entry in &wasm_entries {
            let node_weight = std::cmp::max(entry.baseline_weight, entry.target_weight);
            if node_weight == 0 {
                continue;
            }
            let bar_w = if global_wasm_weight > 0 {
                (node_weight as f64 / global_wasm_weight as f64) * CHART_WIDTH
            } else {
                CHART_WIDTH / wasm_entries.len() as f64
            };
            if bar_w < MIN_BAR_PX {
                continue;
            }

            render_diff_bar(&mut body, entry, bar_x, current_y, bar_w - 1.0, BAR_HEIGHT);
            bar_x += bar_w;
        }
        current_y += BAR_HEIGHT + BAR_GAP;
    }

    let total_height = current_y + 60.0;
    let mut svg = String::new();

    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}" style="background-color:#0d1117">"##,
        SVG_WIDTH, total_height, SVG_WIDTH, total_height
    ));

    svg.push_str(
        r##"  <defs>
    <linearGradient id="grad-stable" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#475569" />
      <stop offset="100%" stop-color="#1e293b" />
    </linearGradient>
    <linearGradient id="grad-regress" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#f87171" />
      <stop offset="100%" stop-color="#991b1b" />
    </linearGradient>
    <linearGradient id="grad-improve" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#4ade80" />
      <stop offset="100%" stop-color="#166534" />
    </linearGradient>
  </defs>
  <style>
    .label   { font-family: 'Inter', 'Roboto Mono', monospace; fill: #f1f5f9; font-size: 11px; dominant-baseline: middle; pointer-events: none; }
    .legend  { font-family: 'Inter', 'Roboto Mono', monospace; fill: #94a3b8; font-size: 10px; dominant-baseline: middle; }
    .sep-txt { font-family: 'Inter', 'Roboto Mono', monospace; fill: #f59e0b; font-size: 10px; font-weight: 600; dominant-baseline: middle; letter-spacing: 0.08em; }
    .box-stable { fill: url(#grad-stable); stroke: #0f172a; stroke-width: 0.5; transition: all 0.1s ease; }
    .box-regress{ fill: url(#grad-regress); stroke: #7f1d1d; stroke-width: 0.5; transition: all 0.1s ease; }
    .box-improve{ fill: url(#grad-improve); stroke: #14532d; stroke-width: 0.5; transition: all 0.1s ease; }
    rect:hover { stroke: #ffffff; stroke-width: 1.5; cursor: pointer; filter: brightness(1.2); }
  </style>"##
    );

    // Title
    svg.push_str(&format!(
        r##"<text x="{}" y="30" font-size="16" fill="#e2e8f0" font-family="'Inter', 'Roboto Mono', monospace" text-anchor="middle" font-weight="bold">Atupa Visual Diff Flamegraph</text>"##,
        SVG_WIDTH / 2.0
    ));

    // Legend
    render_diff_legend(&mut svg, total_height - 20.0);

    svg.push_str(&body);
    svg.push_str("</svg>");
    Ok(svg)
}

// ─── Private Helpers ──────────────────────────────────────────────────────────

fn merge_diff_entries(
    baseline_stacks: &[CollapsedStack],
    target_stacks: &[CollapsedStack],
) -> HashMap<String, DiffEntry> {
    let mut merged: HashMap<String, DiffEntry> = HashMap::new();

    for s in baseline_stacks {
        merged.insert(
            s.stack.clone(),
            DiffEntry {
                stack: s.stack.clone(),
                depth: s.depth,
                vm_kind: s.vm_kind.clone(),
                baseline_weight: s.weight,
                target_weight: 0,
                resolved_label: s.resolved_label.clone(),
                target_address: s.target_address.clone(),
                reverted: s.reverted,
            },
        );
    }

    for s in target_stacks {
        if let Some(entry) = merged.get_mut(&s.stack) {
            entry.target_weight += s.weight;
        } else {
            merged.insert(
                s.stack.clone(),
                DiffEntry {
                    stack: s.stack.clone(),
                    depth: s.depth,
                    vm_kind: s.vm_kind.clone(),
                    baseline_weight: 0,
                    target_weight: s.weight,
                    resolved_label: s.resolved_label.clone(),
                    target_address: s.target_address.clone(),
                    reverted: s.reverted,
                },
            );
        }
    }

    merged
}

fn render_diff_bar(out: &mut String, entry: &DiffEntry, x: f64, y: f64, w: f64, h: f64) {
    let baseline = entry.baseline_weight;
    let target = entry.target_weight;

    let class = get_diff_class(baseline, target);
    let tooltip = format_diff_tooltip(entry);

    out.push_str(&format!(
        r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="4" class="{}">"##,
        x, y, w, h, class
    ));
    out.push_str(&format!(r##"<title>{}</title></rect>"##, tooltip));

    let display_name = get_diff_display_name(entry, w);
    if !display_name.is_empty() {
        out.push_str(&format!(
            r##"<text x="{:.2}" y="{:.2}" class="label">{}</text>"##,
            x + 6.0,
            y + 13.0,
            display_name
        ));
    }
}

pub(crate) fn get_diff_class(baseline: u64, target: u64) -> &'static str {
    if baseline == 0 && target == 0 {
        return "box-stable";
    }
    if baseline == 0 {
        return "box-regress";
    }
    if target == 0 {
        return "box-improve";
    }

    let change = (target as f64 - baseline as f64) / baseline as f64;

    if change > 0.01 {
        "box-regress"
    } else if change < -0.01 {
        "box-improve"
    } else {
        "box-stable"
    }
}

fn format_diff_tooltip(entry: &DiffEntry) -> String {
    let baseline = entry.baseline_weight;
    let target = entry.target_weight;
    let leaf = stack_leaf(&entry.stack);

    let prefix = if entry.reverted { "REVERTED — " } else { "" };
    let vm = match entry.vm_kind {
        VmKind::Stylus => "Stylus",
        _ => "EVM",
    };

    if baseline == 0 {
        return format!("{prefix}{leaf} [{vm}] | NEW: {target} gas");
    }
    if target == 0 {
        return format!("{prefix}{leaf} [{vm}] | REMOVED: {baseline} gas");
    }

    let diff = target as i64 - baseline as i64;
    let percent = (diff as f64 / baseline as f64) * 100.0;

    format!("{prefix}{leaf} [{vm}] | {baseline} -> {target} gas ({percent:+.2}%)")
}

fn get_diff_display_name(entry: &DiffEntry, w: f64) -> String {
    let leaf = stack_leaf(&entry.stack);
    let base = if let Some(r) = &entry.resolved_label {
        r.clone()
    } else if let Some(a) = &entry.target_address {
        format!("{leaf} [{a}]")
    } else {
        format!("{leaf} ({} gas)", entry.target_weight)
    };

    truncate_label(&base, w)
}

fn render_diff_legend(out: &mut String, y: f64) {
    let items = [
        ("Regression (Target &gt; Base)", "box-regress"),
        ("Improvement (Target &lt; Base)", "box-improve"),
        ("No Change", "box-stable"),
    ];

    let start_x = (1000.0 - (items.len() as f64 * 200.0)) / 2.0;

    for (i, (label, class)) in items.iter().enumerate() {
        let x = start_x + (i as f64 * 220.0);
        out.push_str(&format!(
            r##"<rect x="{}" y="{}" width="12" height="12" rx="2" class="{}"/>"##,
            x,
            y - 6.0,
            class
        ));
        out.push_str(&format!(
            r##"<text x="{}" y="{}" class="legend">{}</text>"##,
            x + 18.0,
            y,
            label
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stack(op: &str, weight: u64, depth: u16) -> CollapsedStack {
        CollapsedStack {
            stack: op.to_string(),
            weight,
            last_pc: Some(0),
            depth,
            vm_kind: VmKind::Evm,
            target_address: None,
            resolved_label: None,
            reverted: false,
        }
    }

    #[test]
    fn empty_diff_returns_placeholder() {
        let svg = generate_diff_flamegraph(&[], &[]).unwrap();
        assert!(svg.contains("No execution data found for diff."));
    }

    #[test]
    fn diff_class_determination() {
        assert_eq!(get_diff_class(100, 150), "box-regress");
        assert_eq!(get_diff_class(100, 50), "box-improve");
        assert_eq!(get_diff_class(100, 100), "box-stable");
        assert_eq!(get_diff_class(0, 100), "box-regress");
        assert_eq!(get_diff_class(100, 0), "box-improve");
    }

    #[test]
    fn diff_flamegraph_renders_valid_svg() {
        let base = vec![make_stack("SSTORE", 20_000, 1)];
        let target = vec![make_stack("SSTORE", 15_000, 1)];

        let svg = generate_diff_flamegraph(&base, &target).unwrap();
        assert!(svg.contains("Atupa Visual Diff Flamegraph"));
        assert!(svg.contains("box-improve"));
    }
}
