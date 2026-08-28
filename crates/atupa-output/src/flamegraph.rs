//! Depth-lane SVG flamegraph generation for single-transaction executions.

use askama::Template;
use atupa_core::{CollapsedStack, VmKind};

use crate::common::{
    render_empty_svg, stack_leaf, truncate_label, BAR_GAP, BAR_HEIGHT, CHART_WIDTH, HEADER_HEIGHT,
    MIN_BAR_PX, PADDING_LEFT, SEPARATOR_HEIGHT, SVG_WIDTH,
};

// ─── Template Types ───────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "flamegraph.svg")]
struct FlamegraphTemplate {
    stacks: Vec<StackEntry>,
    width: u32,
    height: u32,
    has_wasm: bool,
    has_starknet: bool,
    has_solana: bool,
    has_stellar: bool,
}

struct StackEntry {
    x: f64,
    y: f64,
    bar_width: f64,
    label: String,
    tooltip: String,
    class: String,
    /// True for the very first Stylus/WASM bar — renderer draws a separator above it.
    is_wasm_section_start: bool,
    /// y-coordinate of the separator line (meaningful when `is_wasm_section_start` is true).
    separator_y: f64,
}

// ─── Renderer ────────────────────────────────────────────────────────────────

/// Generates visual SVG flamegraphs from aggregated execution stacks.
pub struct SvgGenerator;

impl SvgGenerator {
    /// Generates a depth-lane, multi-VM SVG flamegraph.
    ///
    /// ## Layout Rules
    /// - EVM (and non-WASM) stacks are arranged in horizontal swim lanes by call depth.
    ///   Deeper calls are placed in lower lanes so visual nesting matches the call hierarchy.
    /// - Within each depth lane, bars are laid out left-to-right proportional to their weight.
    /// - Stylus/WASM HostIO steps render below a separator in a dedicated amber lane.
    /// - Reverted stacks use a distinct red gradient.
    pub fn generate_flamegraph(stacks: &[CollapsedStack]) -> anyhow::Result<String> {
        if stacks.is_empty() || stacks.iter().all(|s| s.weight == 0) {
            return Ok(render_empty_svg("No execution data found."));
        }

        let evm_stacks: Vec<&CollapsedStack> = stacks
            .iter()
            .filter(|s| s.vm_kind != VmKind::Stylus)
            .collect();
        let wasm_stacks: Vec<&CollapsedStack> = stacks
            .iter()
            .filter(|s| s.vm_kind == VmKind::Stylus)
            .collect();
        let has_wasm = !wasm_stacks.is_empty();

        let global_evm_weight: u64 = evm_stacks.iter().map(|s| s.weight).sum();
        let global_wasm_weight: u64 = wasm_stacks.iter().map(|s| s.weight).sum();

        let mut entries: Vec<StackEntry> = Vec::new();
        let mut current_y = HEADER_HEIGHT;

        // 1. Layout standard depth lanes (EVM / non-Stylus)
        layout_depth_lanes(
            &evm_stacks,
            global_evm_weight,
            &mut entries,
            &mut current_y,
        );

        // 2. Layout Stylus/WASM section if present
        if has_wasm {
            layout_wasm_section(
                &wasm_stacks,
                global_wasm_weight,
                &mut entries,
                &mut current_y,
            );
        }

        let has_starknet = evm_stacks.iter().any(|s| s.vm_kind == VmKind::Starknet);
        let has_solana = evm_stacks.iter().any(|s| s.vm_kind == VmKind::Solana);
        let has_stellar = evm_stacks.iter().any(|s| s.vm_kind == VmKind::Stellar);

        let height = (current_y + 16.0) as u32;
        let template = FlamegraphTemplate {
            stacks: entries,
            width: SVG_WIDTH as u32,
            height,
            has_wasm,
            has_starknet,
            has_solana,
            has_stellar,
        };

        Ok(template.render()?)
    }
}

// ─── Private Layout Helpers ───────────────────────────────────────────────────

fn layout_depth_lanes(
    evm_stacks: &[&CollapsedStack],
    global_weight: u64,
    entries: &mut Vec<StackEntry>,
    current_y: &mut f64,
) {
    let mut depths: Vec<u16> = evm_stacks.iter().map(|s| s.depth).collect();
    depths.sort_unstable();
    depths.dedup();

    for depth in &depths {
        let lane_stacks: Vec<&&CollapsedStack> =
            evm_stacks.iter().filter(|s| s.depth == *depth).collect();
        let lane_weight: u64 = lane_stacks.iter().map(|s| s.weight).sum();
        if lane_weight == 0 {
            continue;
        }

        let mut bar_x = PADDING_LEFT;
        for stack in &lane_stacks {
            if stack.weight == 0 {
                continue;
            }
            let bar_w = (stack.weight as f64 / lane_weight as f64) * CHART_WIDTH;
            if bar_w < MIN_BAR_PX {
                continue;
            }

            let class = get_stack_css_class(stack);
            let label = make_bar_label(stack, bar_w);
            let tooltip = build_tooltip(stack, global_weight);

            entries.push(StackEntry {
                x: bar_x,
                y: *current_y,
                bar_width: bar_w - 1.0, // 1px breathing gap between siblings
                label,
                tooltip,
                class,
                is_wasm_section_start: false,
                separator_y: 0.0,
            });
            bar_x += bar_w;
        }

        *current_y += BAR_HEIGHT + BAR_GAP;
    }
}

fn layout_wasm_section(
    wasm_stacks: &[&CollapsedStack],
    global_weight: u64,
    entries: &mut Vec<StackEntry>,
    current_y: &mut f64,
) {
    *current_y += SEPARATOR_HEIGHT;
    let mut bar_x = PADDING_LEFT;

    for stack in wasm_stacks {
        if stack.weight == 0 {
            continue;
        }
        let bar_w = if global_weight > 0 {
            (stack.weight as f64 / global_weight as f64) * CHART_WIDTH
        } else {
            CHART_WIDTH / wasm_stacks.len() as f64
        };
        if bar_w < MIN_BAR_PX {
            continue;
        }

        let label = make_bar_label(stack, bar_w);
        let pct = if global_weight > 0 {
            stack.weight as f64 / global_weight as f64 * 100.0
        } else {
            0.0
        };
        let tooltip = format!(
            "{} | Stylus HostIO | {:.2} gas-equiv ({:.1}%)",
            stack_leaf(&stack.stack),
            stack.weight as f64,
            pct
        );

        let is_first_wasm = entries.iter().all(|e| e.class != "box-wasm");
        entries.push(StackEntry {
            x: bar_x,
            y: *current_y,
            bar_width: bar_w - 1.0,
            label,
            tooltip,
            class: "box-wasm".to_string(),
            is_wasm_section_start: is_first_wasm,
            separator_y: *current_y - 18.0,
        });
        bar_x += bar_w;
    }

    *current_y += BAR_HEIGHT + BAR_GAP;
}

fn get_stack_css_class(stack: &CollapsedStack) -> String {
    if stack.reverted {
        "box-revert".to_string()
    } else {
        match stack.vm_kind {
            VmKind::Starknet => "box-starknet".to_string(),
            VmKind::Solana => "box-solana".to_string(),
            VmKind::Stellar => "box-stellar".to_string(),
            _ => "box-evm".to_string(),
        }
    }
}

fn make_bar_label(stack: &CollapsedStack, bar_w: f64) -> String {
    let base = if let Some(r) = &stack.resolved_label {
        r.clone()
    } else if let Some(addr) = &stack.target_address {
        format!("{} [{}]", stack_leaf(&stack.stack), addr)
    } else {
        format!("{} ({} gas)", stack_leaf(&stack.stack), stack.weight)
    };

    truncate_label(&base, bar_w)
}

fn build_tooltip(stack: &CollapsedStack, global_weight: u64) -> String {
    let pct = if global_weight > 0 {
        stack.weight as f64 / global_weight as f64 * 100.0
    } else {
        0.0
    };

    let leaf = stack_leaf(&stack.stack);
    if stack.reverted {
        format!(
            "REVERTED — {} | depth {} | {} gas ({:.1}%)",
            leaf, stack.depth, stack.weight, pct
        )
    } else {
        format!(
            "{} | depth {} | {} gas ({:.1}%)",
            leaf, stack.depth, stack.weight, pct
        )
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn evm_stack() -> CollapsedStack {
        CollapsedStack {
            stack: "CALL".to_string(),
            weight: 21_000,
            last_pc: Some(0),
            depth: 1,
            vm_kind: VmKind::Evm,
            target_address: None,
            resolved_label: None,
            reverted: false,
        }
    }

    fn stack_with_vm(vm_kind: VmKind) -> CollapsedStack {
        CollapsedStack {
            stack: "OP".to_string(),
            weight: 1_000,
            last_pc: Some(0),
            depth: 1,
            vm_kind,
            target_address: None,
            resolved_label: None,
            reverted: false,
        }
    }

    #[test]
    fn empty_stacks_returns_placeholder() {
        let svg = SvgGenerator::generate_flamegraph(&[]).unwrap();
        assert!(svg.contains("No execution data found."));
    }

    #[test]
    fn zero_weight_stacks_returns_placeholder() {
        let mut stack = evm_stack();
        stack.weight = 0;
        let svg = SvgGenerator::generate_flamegraph(&[stack]).unwrap();
        assert!(svg.contains("No execution data found."));
    }

    #[test]
    fn legend_pure_evm_shows_revert_not_solana() {
        let stacks = vec![evm_stack()];
        let svg = SvgGenerator::generate_flamegraph(&stacks).expect("SVG generated");

        assert!(svg.contains(r#"class="box-evm""#));
        assert!(!svg.contains(r#"class="box-solana""#));
        assert!(svg.contains(r#"class="box-revert""#));
    }

    #[test]
    fn legend_starknet_trace() {
        let stacks = vec![stack_with_vm(VmKind::Starknet)];
        let svg = SvgGenerator::generate_flamegraph(&stacks).expect("SVG generated");

        assert!(svg.contains(r#"class="box-starknet""#));
        assert!(!svg.contains(r#"class="box-solana""#));
        assert!(!svg.contains(r#"class="box-stellar""#));
    }

    #[test]
    fn legend_solana_trace() {
        let stacks = vec![stack_with_vm(VmKind::Solana)];
        let svg = SvgGenerator::generate_flamegraph(&stacks).expect("SVG generated");

        assert!(svg.contains(r#"class="box-solana""#));
        assert!(!svg.contains(r#"class="box-starknet""#));
    }

    #[test]
    fn legend_stellar_trace() {
        let stacks = vec![stack_with_vm(VmKind::Stellar)];
        let svg = SvgGenerator::generate_flamegraph(&stacks).expect("SVG generated");

        assert!(svg.contains(r#"class="box-stellar""#));
        assert!(!svg.contains(r#"class="box-solana""#));
    }

    #[test]
    fn legend_stylus_trace() {
        let stacks = vec![stack_with_vm(VmKind::Stylus)];
        let svg = SvgGenerator::generate_flamegraph(&stacks).expect("SVG generated");

        assert!(svg.contains(r#"class="box-wasm""#));
        assert!(!svg.contains(r#"class="box-solana""#));
    }
}
