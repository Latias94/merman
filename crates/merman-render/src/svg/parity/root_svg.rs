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

pub(super) enum SvgRootAriaAttrOrder {
    DescribedbyThenLabelledby,
    LabelledbyThenDescribedby,
}

pub(super) enum SvgRootFixedHeightPlacement {
    BeforeXmlns,
    AfterXmlns,
    AfterClass,
}

pub(super) struct SvgRootAttrs<'a> {
    pub(super) diagram_id: &'a str,
    pub(super) class: Option<&'a str>,
    pub(super) width: SvgRootWidth<'a>,
    pub(super) height_attr: Option<&'a str>,
    pub(super) style_attr: Option<&'a str>,
    pub(super) viewbox_attr: Option<&'a str>,
    pub(super) style_viewbox_order: SvgRootStyleViewBoxOrder,
    pub(super) extra_attrs: &'a [(&'a str, &'a str)],
    pub(super) aria_roledescription: &'a str,
    pub(super) aria_labelledby: Option<&'a str>,
    pub(super) aria_describedby: Option<&'a str>,
    pub(super) after_roledescription_attrs: &'a [(&'a str, &'a str)],
    pub(super) tail_attrs: &'a [(&'a str, &'a str)],
    pub(super) fixed_height_placement: SvgRootFixedHeightPlacement,
    pub(super) trailing_newline: bool,
    pub(super) aria_attr_order: SvgRootAriaAttrOrder,
}

impl<'a> SvgRootAttrs<'a> {
    pub(super) fn new(diagram_id: &'a str, aria_roledescription: &'a str) -> Self {
        Self {
            diagram_id,
            class: None,
            width: SvgRootWidth::None,
            height_attr: None,
            style_attr: None,
            viewbox_attr: None,
            style_viewbox_order: SvgRootStyleViewBoxOrder::StyleThenViewBox,
            extra_attrs: &[],
            aria_roledescription,
            aria_labelledby: None,
            aria_describedby: None,
            after_roledescription_attrs: &[],
            tail_attrs: &[],
            fixed_height_placement: SvgRootFixedHeightPlacement::BeforeXmlns,
            trailing_newline: true,
            aria_attr_order: SvgRootAriaAttrOrder::DescribedbyThenLabelledby,
        }
    }
}

pub(super) fn push_svg_root_open(out: &mut String, attrs: SvgRootAttrs<'_>) {
    let SvgRootAttrs {
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
        after_roledescription_attrs,
        tail_attrs,
        fixed_height_placement,
        trailing_newline,
        aria_attr_order,
    } = attrs;

    // Keep attribute order stable (helps strict-mode diffs) and match existing renderers:
    // id, width/height (with configurable fixed-height placement), xmlns, class?,
    // style?/viewBox (configurable), extra-attrs..., role, aria-roledescription, aria-*, tail-attrs..., >\n?
    let mut deferred_height_after_class: Option<&str> = None;
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
                SvgRootFixedHeightPlacement::AfterClass => {
                    out.push_str(
                        r#" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
                    );
                    deferred_height_after_class = Some(height_attr.unwrap_or("0"));
                }
            }
        }
    }

    if let Some(class) = class {
        out.push_str(r#" class=""#);
        out.push_str(class);
        out.push('"');
    }
    if let Some(h) = deferred_height_after_class.take() {
        out.push_str(r#" height=""#);
        out.push_str(h);
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
            }
        }
        SvgRootStyleViewBoxOrder::ViewBoxThenStyle => {
            if let Some(viewbox_attr) = viewbox_attr {
                out.push_str(r#" viewBox=""#);
                out.push_str(viewbox_attr);
                out.push('"');
            }
            if let Some(style_attr) = style_attr {
                out.push_str(r#" style=""#);
                out.push_str(style_attr);
                out.push('"');
            }
        }
    }
    if let Some(h) = deferred_height_after_class.take() {
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
    for (k, v) in after_roledescription_attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str(r#"=""#);
        out.push_str(v);
        out.push('"');
    }
    match aria_attr_order {
        SvgRootAriaAttrOrder::DescribedbyThenLabelledby => {
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
        }
        SvgRootAriaAttrOrder::LabelledbyThenDescribedby => {
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
        }
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
