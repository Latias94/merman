//! Shared RoughJS formatting and path-geometry helpers used by multiple parity renderers.

#[cfg(any(
    feature = "diagram-c4",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(in crate::svg::parity) use merman_render_core::rough_svg::paths;

pub(in crate::svg::parity) use merman_render_core::rough_svg::{
    closed_path_d_from_points, roughjs_circle_operations, roughjs_rect_operations,
};

#[cfg(test)]
mod color_tests {
    use merman_render_core::rough_svg::parse_hex_color_to_srgba;

    #[test]
    fn malformed_hex_color_never_slices_inside_utf8() {
        for value in ["#😀ab", "#éa", "#猫", "#ééé", "#abcdefg", "#ggg"] {
            assert!(parse_hex_color_to_srgba(value).is_none(), "{value}");
        }
        for value in ["#abc", "#ABCDEF", " #ff0000 ", "red", "hsl(180 50% 50%)"] {
            assert!(parse_hex_color_to_srgba(value).is_some(), "{value}");
        }
    }
}
