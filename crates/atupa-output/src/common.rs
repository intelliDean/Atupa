//! Shared layout constants and rendering helpers for SVG flamegraph generation.

/// Standard total canvas width for generated SVGs in pixels.
pub const SVG_WIDTH: f64 = 1000.0;

/// Left and right padding inside the SVG canvas.
pub const PADDING_LEFT: f64 = 10.0;

/// Usable chart width for rendering stack bars.
pub const CHART_WIDTH: f64 = SVG_WIDTH - PADDING_LEFT * 2.0;

/// Height of a single stack bar in pixels.
pub const BAR_HEIGHT: f64 = 26.0;

/// Vertical gap between adjacent depth lanes in pixels.
pub const BAR_GAP: f64 = 4.0;

/// Top header space for legend and title in standard flamegraphs.
pub const HEADER_HEIGHT: f64 = 36.0;

/// Top header space in diff flamegraphs.
pub const DIFF_HEADER_HEIGHT: f64 = 60.0;

/// Height reserved for the EVM/WASM divider row.
pub const SEPARATOR_HEIGHT: f64 = 28.0;

/// Minimum pixel width required to render a bar (prevents 0-width visual artifacts).
pub const MIN_BAR_PX: f64 = 2.0;

/// Approximate horizontal character width for font rendering calculations (Inter @ 11px ≈ 7.0px).
const CHAR_WIDTH_PX: f64 = 7.0;

/// Renders a fallback SVG canvas displaying an informational message.
pub fn render_empty_svg(message: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 60" style="background-color:#0d1117"><text x="14" y="34" fill="#94a3b8" font-family="Inter, monospace" font-size="13">{}</text></svg>"##,
        message
    )
}

/// Truncates a text label with an ellipsis (`…`) so that it fits comfortably
/// inside a bar of width `bar_width` pixels.
pub fn truncate_label(text: &str, bar_width: f64) -> String {
    let max_chars = ((bar_width - 8.0) / CHAR_WIDTH_PX) as usize;
    if max_chars < 3 {
        return String::new();
    }
    if text.len() <= max_chars {
        text.to_string()
    } else {
        format!("{}…", &text[..max_chars.saturating_sub(1)])
    }
}

/// Extracts the leaf opcode or label from a semi-colon delimited stack path.
///
/// E.g. `"CALL;SSTORE;KECCAK256"` -> `"KECCAK256"`.
#[inline]
pub fn stack_leaf(stack: &str) -> &str {
    stack.split(';').next_back().unwrap_or(stack)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_leaf_extraction() {
        assert_eq!(stack_leaf("CALL;SSTORE;KECCAK256"), "KECCAK256");
        assert_eq!(stack_leaf("SINGLE_OP"), "SINGLE_OP");
        assert_eq!(stack_leaf(""), "");
    }

    #[test]
    fn truncate_label_behavior() {
        // Wide bar: no truncation
        assert_eq!(truncate_label("submit", 100.0), "submit");

        // Medium bar: truncates with ellipsis
        let truncated = truncate_label("very_long_function_name_that_overflows", 70.0);
        assert!(truncated.ends_with('…'));
        assert!(truncated.len() < "very_long_function_name_that_overflows".len());

        // Narrow bar (< 3 chars): returns empty string
        assert_eq!(truncate_label("submit", 15.0), "");
    }

    #[test]
    fn render_empty_svg_contains_message() {
        let svg = render_empty_svg("No execution data found.");
        assert!(svg.contains("No execution data found."));
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
