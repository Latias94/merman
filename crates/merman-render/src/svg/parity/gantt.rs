mod render;

pub(super) use render::render_gantt_diagram_svg_model;

pub(super) fn canonical_gantt_css(
    diagram_id: impl Copy + std::fmt::Display,
    effective_config: &serde_json::Value,
) -> String {
    let sanitized_id = super::sanitize_svg_id(&diagram_id.to_string());
    let mut css = super::gantt_css(diagram_id, effective_config);
    // The canonical adapter expands milestone bars into already-rotated path geometry.  Keep the
    // source class for DOM consumers, but prevent the legacy CSS transform from rotating it twice.
    css.push_str(&format!("#{} .milestone{{transform:none;}}", sanitized_id));
    css
}
