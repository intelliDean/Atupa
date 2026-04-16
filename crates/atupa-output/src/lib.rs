use askama::Template;
use atupa_core::{CollapsedStack, VmKind};

// ─── Template types ──────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "flamegraph.svg")]
struct FlamegraphTemplate {
    stacks: Vec<StackEntry>,
    width: u32,
    height: u32,
    has_wasm: bool,
}

struct StackEntry {
    x: f64,
    y: f64,
    bar_width: f64,
    label: String,
    tooltip: String,
    class: String,
    /// True for the very first Stylus/WASM bar — renderer draws separator above it.
    is_wasm_section_start: bool,
    /// y-coordinate of the separator line (only meaningful when is_wasm_section_start)
    separator_y: f64,
}

// ─── Renderer ────────────────────────────────────────────────────────────────

pub struct SvgGenerator;

impl SvgGenerator {
    /// Generates a depth-lane, dual-VM SVG flamegraph.
    ///
    /// Layout rules:
    /// - EVM stacks are arranged in horizontal swim lanes by call depth.
    ///   Deeper calls are placed in lower lanes so the visual nesting matches
    ///   the actual call hierarchy.
    /// - Within each depth lane the bars are laid out left-to-right proportional
    ///   to their gas weight.
    /// - Stylus/WASM HostIO steps render below a separator in a dedicated amber lane.
    /// - Reverted stacks use a red gradient.
    pub fn generate_flamegraph(stacks: &[CollapsedStack]) -> anyhow::Result<String> {
        if stacks.is_empty() || stacks.iter().all(|s| s.weight == 0) {
            return Ok("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 60\" \
                       style=\"background-color:#0d1117\">\
                       <text x=\"14\" y=\"34\" fill=\"#94a3b8\" \
                       font-family=\"Inter,monospace\" font-size=\"13\">\
                       No execution data found.\
                       </text></svg>"
                .to_string());
        }

        const SVG_W: f64 = 1000.0;
        const PAD_L: f64 = 10.0;
        const CHART_W: f64 = SVG_W - PAD_L * 2.0;
        const BAR_H: f64 = 26.0;
        const GAP: f64 = 4.0;
        const HEADER_H: f64 = 36.0; // row for legend + title
        const SEPARATOR_H: f64 = 28.0; // height of the EVM/WASM divider row
        const MIN_BAR_PX: f64 = 2.0;

        let evm_stacks: Vec<&CollapsedStack> =
            stacks.iter().filter(|s| s.vm_kind == VmKind::Evm).collect();
        let wasm_stacks: Vec<&CollapsedStack> =
            stacks.iter().filter(|s| s.vm_kind == VmKind::Stylus).collect();
        let has_wasm = !wasm_stacks.is_empty();

        // Gather unique depths for EVM stacks, sorted ascending (depth 1 on top).
        let mut depths: Vec<u16> = evm_stacks.iter().map(|s| s.depth).collect();
        depths.sort_unstable();
        depths.dedup();

        // Total EVM weight is per depth-lane (each lane fills CHART_W independently)
        // but we need the global total for the tooltip percentage.
        let global_evm_weight: u64 = evm_stacks.iter().map(|s| s.weight).sum();
        let global_wasm_weight: u64 = wasm_stacks.iter().map(|s| s.weight).sum();

        let mut entries: Vec<StackEntry> = Vec::new();
        let mut current_y = HEADER_H;

        // ── EVM depth lanes ───────────────────────────────────────────────────
        for depth in &depths {
            let lane_stacks: Vec<&&CollapsedStack> = evm_stacks
                .iter()
                .filter(|s| s.depth == *depth)
                .collect();
            let lane_weight: u64 = lane_stacks.iter().map(|s| s.weight).sum();
            if lane_weight == 0 {
                continue;
            }

            let mut bar_x = PAD_L;
            for stack in &lane_stacks {
                if stack.weight == 0 {
                    continue;
                }
                let bar_w = (stack.weight as f64 / lane_weight as f64) * CHART_W;
                if bar_w < MIN_BAR_PX {
                    continue;
                }

                let class = if stack.reverted {
                    "box-revert"
                } else {
                    "box-evm"
                };
                let label =
                    Self::make_label(stack, bar_w);
                let pct = if global_evm_weight > 0 {
                    stack.weight as f64 / global_evm_weight as f64 * 100.0
                } else {
                    0.0
                };
                let tooltip = if stack.reverted {
                    format!(
                        "REVERTED — {} | depth {} | {} gas ({:.1}%)",
                        Self::stack_leaf(stack),
                        stack.depth,
                        stack.weight,
                        pct
                    )
                } else {
                    format!(
                        "{} | depth {} | {} gas ({:.1}%)",
                        Self::stack_leaf(stack),
                        stack.depth,
                        stack.weight,
                        pct
                    )
                };

                entries.push(StackEntry {
                    x: bar_x,
                    y: current_y,
                    bar_width: bar_w - 1.0, // 1px breathing gap between siblings
                    label,
                    tooltip,
                    class: class.to_string(),
                    is_wasm_section_start: false,
                    separator_y: 0.0,
                });
                bar_x += bar_w;
            }

            current_y += BAR_H + GAP;
        }

        // ── WASM section ─────────────────────────────────────────────────────
        if has_wasm {
            // Spacer / label row — rendered via template has_wasm flag not via entries
            current_y += SEPARATOR_H;

            let mut bar_x = PAD_L;
            for stack in &wasm_stacks {
                if stack.weight == 0 {
                    continue;
                }
                let bar_w = if global_wasm_weight > 0 {
                    (stack.weight as f64 / global_wasm_weight as f64) * CHART_W
                } else {
                    CHART_W / wasm_stacks.len() as f64
                };
                if bar_w < MIN_BAR_PX {
                    continue;
                }

                let label = Self::make_label(stack, bar_w);
                let pct = if global_wasm_weight > 0 {
                    stack.weight as f64 / global_wasm_weight as f64 * 100.0
                } else {
                    0.0
                };
                let tooltip = format!(
                    "{} | Stylus HostIO | {:.2} gas-equiv ({:.1}%)",
                    Self::stack_leaf(stack),
                    stack.weight as f64,
                    pct
                );

                let is_first_wasm = entries.iter().all(|e| e.class != "box-wasm");
                entries.push(StackEntry {
                    x: bar_x,
                    y: current_y,
                    bar_width: bar_w - 1.0,
                    label,
                    tooltip,
                    class: "box-wasm".to_string(),
                    is_wasm_section_start: is_first_wasm,
                    separator_y: current_y - 18.0,
                });
                bar_x += bar_w;
            }

            current_y += BAR_H + GAP;
        }

        let height = (current_y + 16.0) as u32;
        let template = FlamegraphTemplate {
            stacks: entries,
            width: SVG_W as u32,
            height,
            has_wasm,
        };
        Ok(template.render()?)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Label shown inside the bar. Uses resolved_label if present, otherwise
    /// builds "LEAF (N gas)". Truncates to fit the available pixel width.
    fn make_label(stack: &CollapsedStack, bar_w: f64) -> String {
        let base = if let Some(r) = &stack.resolved_label {
            r.clone()
        } else if let Some(addr) = &stack.target_address {
            format!("{} [{}]", Self::stack_leaf(stack), addr)
        } else {
            format!("{} ({} gas)", Self::stack_leaf(stack), stack.weight)
        };

        // Approximate character fit: Inter/mono ≈ 7px per char at 12px
        let max_chars = ((bar_w - 8.0) / 7.0) as usize;
        if max_chars < 3 {
            return String::new();
        }
        if base.len() <= max_chars {
            base
        } else {
            format!("{}…", &base[..max_chars.saturating_sub(1)])
        }
    }

    fn stack_leaf(stack: &CollapsedStack) -> &str {
        stack.stack.split(';').next_back().unwrap_or(&stack.stack)
    }
}
