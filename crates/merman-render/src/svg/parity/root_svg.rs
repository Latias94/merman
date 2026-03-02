use super::*;

pub(super) enum SvgRootWidth<'a> {
    None,
    Percent100,
    Fixed(&'a str),
}

pub(super) enum SvgRootStyleViewBoxOrder {
    StyleThenViewBox,
    ViewBoxThenStyle,
}

pub(super) enum SvgRootFixedHeightPlacement {
    BeforeXmlns,
    AfterXmlns,
    AfterViewBox,
}

pub(super) fn push_svg_root_open_ex(
    out: &mut String,
    diagram_id: &str,
    class: Option<&str>,
    width: SvgRootWidth<'_>,
    height_attr: Option<&str>,
    style_attr: Option<&str>,
    viewbox_attr: Option<&str>,
    style_viewbox_order: SvgRootStyleViewBoxOrder,
    extra_attrs: &[(&str, &str)],
    aria_roledescription: &str,
    aria_labelledby: Option<&str>,
    aria_describedby: Option<&str>,
    trailing_newline: bool,
) {
    push_svg_root_open_ex2(
        out,
        diagram_id,
        class,
        width,
        height_attr,
        style_attr,
        viewbox_attr,
        style_viewbox_order,
        extra_attrs,
        aria_roledescription,
        aria_labelledby,
        aria_describedby,
        &[],
        SvgRootFixedHeightPlacement::BeforeXmlns,
        trailing_newline,
    );
}

pub(super) fn push_svg_root_open_ex2(
    out: &mut String,
    diagram_id: &str,
    class: Option<&str>,
    width: SvgRootWidth<'_>,
    height_attr: Option<&str>,
    style_attr: Option<&str>,
    viewbox_attr: Option<&str>,
    style_viewbox_order: SvgRootStyleViewBoxOrder,
    extra_attrs: &[(&str, &str)],
    aria_roledescription: &str,
    aria_labelledby: Option<&str>,
    aria_describedby: Option<&str>,
    tail_attrs: &[(&str, &str)],
    fixed_height_placement: SvgRootFixedHeightPlacement,
    trailing_newline: bool,
) {
    // Keep attribute order stable (helps strict-mode diffs) and match existing renderers:
    // id, width/height (with configurable fixed-height placement), xmlns, class?,
    // style?/viewBox (configurable), extra-attrs..., role, aria-roledescription, aria-*, tail-attrs..., >\n?
    let mut deferred_height: Option<&str> = None;
    out.push_str(r#"<svg id=""#);
    escape_xml_into(out, diagram_id);
    match width {
        SvgRootWidth::None => {
            out.push_str(r#"" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#);
        }
        SvgRootWidth::Percent100 => {
            out.push_str(
                r#"" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
            );
        }
        SvgRootWidth::Fixed(w) => {
            out.push_str(r#"" width=""#);
            out.push_str(w);
            out.push('"');
            match fixed_height_placement {
                SvgRootFixedHeightPlacement::BeforeXmlns => {
                    out.push_str(r#" height=""#);
                    out.push_str(height_attr.unwrap_or("0"));
                    out.push('"');
                    out.push_str(
                        r#" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
                    );
                }
                SvgRootFixedHeightPlacement::AfterXmlns => {
                    out.push_str(
                        r#" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
                    );
                    out.push_str(r#" height=""#);
                    out.push_str(height_attr.unwrap_or("0"));
                    out.push('"');
                }
                SvgRootFixedHeightPlacement::AfterViewBox => {
                    out.push_str(
                        r#" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
                    );
                    deferred_height = Some(height_attr.unwrap_or("0"));
                }
            }
        }
    }

    if let Some(class) = class {
        out.push_str(r#" class=""#);
        out.push_str(class);
        out.push('"');
    }
    match style_viewbox_order {
        SvgRootStyleViewBoxOrder::StyleThenViewBox => {
            if let Some(style_attr) = style_attr {
                out.push_str(r#" style=""#);
                out.push_str(style_attr);
                out.push('"');
            }
            if let Some(viewbox_attr) = viewbox_attr {
                out.push_str(r#" viewBox=""#);
                out.push_str(viewbox_attr);
                out.push('"');
                if let Some(h) = deferred_height.take() {
                    out.push_str(r#" height=""#);
                    out.push_str(h);
                    out.push('"');
                }
            }
        }
        SvgRootStyleViewBoxOrder::ViewBoxThenStyle => {
            if let Some(viewbox_attr) = viewbox_attr {
                out.push_str(r#" viewBox=""#);
                out.push_str(viewbox_attr);
                out.push('"');
                if let Some(h) = deferred_height.take() {
                    out.push_str(r#" height=""#);
                    out.push_str(h);
                    out.push('"');
                }
            }
            if let Some(style_attr) = style_attr {
                out.push_str(r#" style=""#);
                out.push_str(style_attr);
                out.push('"');
            }
        }
    }
    if let Some(h) = deferred_height.take() {
        out.push_str(r#" height=""#);
        out.push_str(h);
        out.push('"');
    }

    for (k, v) in extra_attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str(r#"=""#);
        out.push_str(v);
        out.push('"');
    }

    out.push_str(r#" role="graphics-document document" aria-roledescription=""#);
    out.push_str(aria_roledescription);
    out.push('"');
    if let Some(v) = aria_describedby {
        out.push_str(r#" aria-describedby=""#);
        out.push_str(v);
        out.push('"');
    }
    if let Some(v) = aria_labelledby {
        out.push_str(r#" aria-labelledby=""#);
        out.push_str(v);
        out.push('"');
    }

    for (k, v) in tail_attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str(r#"=""#);
        out.push_str(v);
        out.push('"');
    }

    out.push('>');
    if trailing_newline {
        out.push('\n');
    }
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
    push_svg_root_open_ex(
        out,
        diagram_id,
        Some(class),
        width,
        height_attr,
        Some(style_attr),
        Some(viewbox_attr),
        SvgRootStyleViewBoxOrder::StyleThenViewBox,
        &[],
        aria_roledescription,
        aria_labelledby,
        aria_describedby,
        true,
    );
}
