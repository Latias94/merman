use super::{InfoDiagramLayout, Result, SvgRenderOptions};
use std::fmt::Write as _;

pub(super) fn render_info_diagram_svg(
    layout: &InfoDiagramLayout,
    _semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = super::escape_xml(diagram_id);

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: 400px; background-color: white;" role="graphics-document document" aria-roledescription="info">"#,
    );
    let css = super::info_css(diagram_id);
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
