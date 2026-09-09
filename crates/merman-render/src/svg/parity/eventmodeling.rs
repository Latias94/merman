mod render;

pub(super) use render::render_eventmodeling_diagram_svg;

use super::super::sanitize_svg_id;
use super::super::theme::PresentationTheme;

pub(super) fn canonical_eventmodeling_css(
    diagram_id: impl Copy + std::fmt::Display,
    effective_config: &serde_json::Value,
) -> String {
    let theme = PresentationTheme::new(effective_config).eventmodeling();
    let scope = format!("#{}", sanitize_svg_id(&diagram_id.to_string()));
    format!(
        "{scope} .em-swimlane text,{scope} .em-box span {{ font-family: {}; color: {}; }}{scope} .em-relation {{ fill: none; }}",
        theme.font_family_css, theme.text_color
    )
}
