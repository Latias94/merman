mod render;

pub(super) use render::render_journey_diagram_svg_model;

pub(super) fn canonical_journey_css(
    diagram_id: impl Copy + std::fmt::Display,
    effective_config: &serde_json::Value,
) -> String {
    let theme = super::theme::PresentationTheme::new(effective_config).journey();
    render::journey_css(diagram_id, effective_config, &theme)
}
