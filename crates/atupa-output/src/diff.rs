use atupa_core::CollapsedStack;
use std::collections::HashMap;

/// Internal DiffNode structure for building the merged tree
struct DiffNode {
    name: String,
    baseline_value: u64,
    target_value: u64,
    children: HashMap<String, DiffNode>,
}

impl DiffNode {
    fn new(name: String) -> Self {
        Self {
            name,
            baseline_value: 0,
            target_value: 0,
            children: HashMap::new(),
        }
    }

    fn insert_baseline(&mut self, stack: &[&str], value: u64) {
        self.baseline_value += value;
        if let Some((head, tail)) = stack.split_first() {
            let child = self
                .children
                .entry(head.to_string())
                .or_insert_with(|| DiffNode::new(head.to_string()));
            child.insert_baseline(tail, value);
        }
    }

    fn insert_target(&mut self, stack: &[&str], value: u64) {
        self.target_value += value;
        if let Some((head, tail)) = stack.split_first() {
            let child = self
                .children
                .entry(head.to_string())
                .or_insert_with(|| DiffNode::new(head.to_string()));
            child.insert_target(tail, value);
        }
    }
}

pub fn generate_diff_flamegraph(
    baseline_stacks: &[CollapsedStack],
    target_stacks: &[CollapsedStack],
) -> anyhow::Result<String> {
    let mut root = DiffNode::new("root".to_string());

    // 1. Build Merged Tree
    for stack in baseline_stacks {
        let mut parts: Vec<&str> = stack.stack.split(';').collect();
        if parts.first() == Some(&"root") {
            parts.remove(0);
        }
        root.insert_baseline(&parts, stack.weight);
    }
    for stack in target_stacks {
        let mut parts: Vec<&str> = stack.stack.split(';').collect();
        if parts.first() == Some(&"root") {
            parts.remove(0);
        }
        root.insert_target(&parts, stack.weight);
    }

    let max_depth = calculate_max_depth(&root);

    // 2. Render SVG
    let mut svg = String::new();
    let width = 1000;
    let height_per_level = 26;
    let graph_height = (max_depth + 1) * height_per_level;
    let legend_height = 80;
    let total_height = graph_height + legend_height + 60;

    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}" style="background-color:#0d1117">"##,
        width, total_height, width, total_height
    ));

    svg.push_str(
        r##"<style>.func { font: 12px Inter, monospace; } .func:hover { stroke: #ffffff; stroke-width: 1.5; cursor: pointer; opacity: 0.9; }</style>"##
    );

    svg.push_str(&format!(
        r##"<text x="{}" y="30" font-size="20" fill="#e2e8f0" font-family="Inter, monospace" text-anchor="middle" font-weight="bold">Atupa Visual Diff Flamegraph</text>"##,
        width / 2
    ));

    let mut ctx = DiffRenderContext {
        output: &mut svg,
        line_height: height_per_level,
        graph_height,
    };

    render_diff_node(&root, 0, 10.0, (width - 20) as f64, &mut ctx);

    render_diff_legend(&mut svg, graph_height + 60);

    svg.push_str("</svg>");
    Ok(svg)
}

fn calculate_max_depth(node: &DiffNode) -> usize {
    if node.children.is_empty() {
        return 0;
    }
    node.children
        .values()
        .map(calculate_max_depth)
        .max()
        .unwrap_or(0)
        + 1
}

struct DiffRenderContext<'a> {
    output: &'a mut String,
    line_height: usize,
    graph_height: usize,
}

fn render_diff_node(node: &DiffNode, level: usize, x: f64, w: f64, ctx: &mut DiffRenderContext) {
    if w < 1.0 {
        return;
    }

    let color = get_diff_color(node.baseline_value, node.target_value);
    let y = (ctx.graph_height as f64)
        - (level as f64 * ctx.line_height as f64)
        - (ctx.line_height as f64)
        + 60.0; // Offset for title

    let tooltip = format_diff_tooltip(node);

    ctx.output.push_str(&format!(
        r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{}" fill="{}" stroke="#1e293b" stroke-width="1.0" class="func" rx="2">"##,
        x, y, w, ctx.line_height - 2, color
    ));
    ctx.output
        .push_str(&format!(r##"<title>{}</title></rect>"##, tooltip));

    let display_name = get_truncated_name(&node.name, w);
    if !display_name.is_empty() {
        ctx.output.push_str(&format!(
            r##"<text x="{:.2}" y="{:.2}" dx="6" dy="16" font-size="12" fill="#f8fafc" font-family="Inter, monospace" style="pointer-events:none">{}</text>"##,
            x, y, display_name
        ));
    }

    // Children: Recurse using max width
    let mut current_x = x;
    let mut children_vec: Vec<&DiffNode> = node.children.values().collect();
    children_vec.sort_by(|a, b| {
        let a_max = a.target_value.max(a.baseline_value);
        let b_max = b.target_value.max(b.baseline_value);
        b_max.cmp(&a_max)
    });

    let parent_max = node.target_value.max(node.baseline_value);

    for child in children_vec {
        let child_max = child.target_value.max(child.baseline_value);
        let child_w = (child_max as f64 / parent_max as f64) * w;
        if child_w >= 1.0 {
            render_diff_node(child, level + 1, current_x, child_w, ctx);
            current_x += child_w;
        }
    }
}

fn get_diff_color(baseline: u64, target: u64) -> String {
    if baseline == 0 && target == 0 {
        return "#334155".into();
    }
    if baseline == 0 {
        return "#ef4444".into(); // New code (Red/Regression)
    } 
    if target == 0 {
        return "#22c55e".into(); // Removed code (Green/Improvement)
    } 

    let change = (target as f64 - baseline as f64) / baseline as f64;

    if change > 0.01 {
        // Red scale for regressions
        let intensity = ((change * 100.0).min(100.0) / 100.0) * 0.5; 
        // Mix between neutral slate and pure red
        format!("rgba(239, 68, 68, {:.2})", 0.5 + intensity)
    } else if change < -0.01 {
        // Green scale for improvements
        let intensity = ((change.abs() * 100.0).min(100.0) / 100.0) * 0.5;
        format!("rgba(34, 197, 94, {:.2})", 0.5 + intensity)
    } else {
        "#475569".into() // Stable/No Change
    }
}

fn format_diff_tooltip(node: &DiffNode) -> String {
    let baseline = node.baseline_value;
    let target = node.target_value;

    if baseline == 0 {
        return format!("{}: {} gas (NEW)", node.name, target);
    }
    if target == 0 {
        return format!("{}: {} gas (REMOVED)", node.name, baseline);
    }

    let diff = target as i64 - baseline as i64;
    let percent = (diff as f64 / baseline as f64) * 100.0;

    format!(
        "{}: {} -> {} gas ({:+.2}%)",
        node.name, baseline, target, percent
    )
}

fn get_truncated_name(name: &str, w: f64) -> String {
    let max_chars = ((w - 12.0) / 7.5) as usize; // approx 7.5px per char for mono font
    if max_chars < 3 {
        return String::new();
    }
    if name.len() <= max_chars {
        name.to_string()
    } else {
        format!("{}…", &name[..max_chars.saturating_sub(1)])
    }
}

fn render_diff_legend(out: &mut String, y: usize) {
    let items = [
        ("Regression (Target > Base)", "#ef4444"),
        ("Improvement (Target < Base)", "#22c55e"),
        ("No Change", "#475569"),
    ];

    for (i, (label, color)) in items.iter().enumerate() {
        let x = 10 + (i * 240);
        out.push_str(&format!(
            r##"<rect x="{}" y="{}" width="18" height="18" fill="{}" rx="4"/>"##,
            x,
            y - 14,
            color
        ));
        out.push_str(&format!(
            r##"<text x="{}" y="{}" font-size="13" fill="#cbd5e1" font-family="Inter, monospace">{}</text>"##,
            x + 26,
            y,
            label
        ));
    }
}
