mod render;

use crate::Result;

pub(super) use render::render_kanban_diagram_svg;

pub(super) fn canonical_kanban_css(
    diagram_id: impl Copy + std::fmt::Display,
    effective_config: &serde_json::Value,
) -> Result<String> {
    render::kanban_css(diagram_id, effective_config)
}
