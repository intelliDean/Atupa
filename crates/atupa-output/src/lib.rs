use askama::Template;
use atupa_core::CollapsedStack;

#[derive(Template)]
#[template(path = "flamegraph.svg")]
struct FlamegraphTemplate {
    stacks: Vec<StackEntry>,
    width: u32,
    height: u32,
}

struct StackEntry {
    y: f64,
    width: f64,
    label: String,
    class: String,
}

pub struct SvgGenerator;

impl SvgGenerator {
    /// Generates a valid SVG visualization string matching Atupa Aesthetic using Askama.
    pub fn generate_flamegraph(stacks: &[CollapsedStack]) -> anyhow::Result<String> {
        let total_weight: u64 = stacks.iter().map(|s| s.weight).sum();
        if total_weight == 0 {
             return Ok("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1000 50\"><text x=\"10\" y=\"30\" fill=\"white\">No execution data found.</text></svg>".to_string());
        }

        let max_width = 980.0;
        let mut current_y = 20.0;
        let mut template_stacks = Vec::new();

        for stack in stacks {
            if stack.weight == 0 {
                continue;
            }
            let width = (stack.weight as f64 / total_weight as f64) * max_width;
            if width < 1.0 {
                continue;
            }

            let box_class = if stack.reverted { "box-revert" } else { "box" };
            let leaf_name = stack.stack.split(';').last().unwrap_or("unknown");
            
            let mut label = format!("{} ({} gas)", leaf_name, stack.weight);
            if let Some(r_label) = &stack.resolved_label {
                label = format!("{} ({} gas)", r_label, stack.weight);
            } else if let Some(addr) = &stack.target_address {
                label = format!("{} [{}] ({} gas)", leaf_name, addr, stack.weight);
            }
            if stack.reverted {
                label = format!("REVERTED: {}", label);
            }

            template_stacks.push(StackEntry {
                y: current_y,
                width,
                label,
                class: box_class.to_string(),
            });

            current_y += 25.0;
        }

        let template = FlamegraphTemplate {
            stacks: template_stacks,
            width: 1000,
            height: (current_y + 20.0) as u32,
        };

        Ok(template.render()?)
    }
}
