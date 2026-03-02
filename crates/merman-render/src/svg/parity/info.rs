use super::{InfoDiagramLayout, Result, SvgRenderOptions, root_svg};
use std::fmt::Write as _;

pub(super) fn render_info_diagram_svg(
    layout: &InfoDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");

    let mut out = String::new();
    root_svg::push_svg_root_open_ex(
        &mut out,
        diagram_id,
        None,
        root_svg::SvgRootWidth::Percent100,
        None,
        Some("max-width: 400px; background-color: white;"),
        None,
        root_svg::SvgRootStyleViewBoxOrder::StyleThenViewBox,
        &[],
        "info",
        None,
        None,
        false,
    );
    let css = super::info_css_with_config(diagram_id, effective_config);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);
    let _ = write!(
        &mut out,
        r#"<g><text x="100" y="40" class="version" font-size="32" style="text-anchor: middle;">{}</text></g>"#,
        super::escape_xml(&layout.version)
    );
    out.push_str("</svg>\n");
    Ok(out)
}
