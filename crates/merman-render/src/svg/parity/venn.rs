mod render;

pub(super) use render::render_venn_diagram_svg_model;

pub(super) fn canonical_venn_css(
    diagram_id: impl Copy + std::fmt::Display,
    effective_config: &serde_json::Value,
) -> crate::Result<String> {
    let theme = super::theme::PresentationTheme::new(effective_config).venn()?;
    Ok(render::venn_css(diagram_id, &theme))
}
