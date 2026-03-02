use super::*;

pub(super) enum SvgRootWidth<'a> {
    Percent100,
    Fixed(&'a str),
}

pub(super) fn push_svg_root_open(
    out: &mut String,
    diagram_id: &str,
    class: &str,
    width: SvgRootWidth<'_>,
    height_attr: Option<&str>,
    style_attr: &str,
    viewbox_attr: &str,
    aria_roledescription: &str,
    aria_labelledby: Option<&str>,
    aria_describedby: Option<&str>,
) {
    // Keep attribute order stable (helps strict-mode diffs) and match existing renderers:
    // id, width/height, xmlns, class, style, viewBox, role, aria-roledescription, aria-*, >\n
    out.push_str(r#"<svg id=""#);
    escape_xml_into(out, diagram_id);
    match width {
        SvgRootWidth::Percent100 => {
            out.push_str(
                r#"" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class=""#,
            );
        }
        SvgRootWidth::Fixed(w) => {
            out.push_str(r#"" width=""#);
            out.push_str(w);
            out.push_str(r#"" height=""#);
            out.push_str(height_attr.unwrap_or("0"));
            out.push_str(
                r#"" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" class=""#,
            );
        }
    }
    out.push_str(class);
    out.push_str(r#"" style=""#);
    out.push_str(style_attr);
    out.push_str(r#"" viewBox=""#);
    out.push_str(viewbox_attr);
    out.push_str(r#"" role="graphics-document document" aria-roledescription=""#);
    out.push_str(aria_roledescription);
    out.push('"');
    if let Some(v) = aria_labelledby {
        out.push_str(r#" aria-labelledby=""#);
        out.push_str(v);
        out.push('"');
    }
    if let Some(v) = aria_describedby {
        out.push_str(r#" aria-describedby=""#);
        out.push_str(v);
        out.push('"');
    }
    out.push('>');
    out.push('\n');
}
