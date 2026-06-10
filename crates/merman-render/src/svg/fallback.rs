#![forbid(unsafe_code)]

// NOTE: This fallback module intentionally keeps parsing "cheap" and non-validating.
// It is a best-effort readability fallback for SVG consumers that do not fully
// support HTML inside `<foreignObject>` (e.g. many rasterizers).

mod attr;
mod context;
mod css;
mod html;
mod xml;

use crate::text::{TextStyle, VendoredFontMetricsTextMeasurer};

use attr::{is_self_closing, parse_attr_f64, parse_attr_str};
use context::{
    GFrame, class_attr_tokens, extract_svg_font_style_from_context,
    extract_svg_text_fill_from_ancestors, fallback_text_class_attr_tokens, sum_translate,
};
use css::extract_css_background_color_for_class;
use html::{
    extract_inline_html_color, extract_inline_html_style_property,
    foreign_object_html_soft_wrap_width, htmlish_to_text_lines, parse_css_px,
    wrap_html_lines_to_width,
};
use xml::{escape_xml_attr, escape_xml_text};

/// Adds a best-effort `<text>/<tspan>` overlay extracted from Mermaid label `<foreignObject>`
/// content.
///
/// Many headless SVG renderers and rasterizers do not fully support HTML inside `<foreignObject>`.
/// The returned SVG aims to be *more readable* for raster outputs and UI previews.
///
/// Important:
/// - This does not aim for Mermaid DOM parity.
/// - For parity-focused SVG output, keep the original SVG unchanged.
pub fn foreign_object_label_fallback_svg_text(svg: &str) -> String {
    if !svg.contains("<foreignObject") {
        return svg.to_string();
    }

    let close_tag = "</foreignObject>";
    let mut out = String::with_capacity(svg.len() + 2048);
    let mut overlays = String::new();
    let mut g_stack: Vec<GFrame> = Vec::new();
    let label_bkg_default = "rgba(232, 232, 232, 0.5)".to_string();
    let label_bkg =
        extract_css_background_color_for_class(svg, "labelBkg").unwrap_or(label_bkg_default);
    let fallback_measurer = VendoredFontMetricsTextMeasurer::default();

    let mut i = 0usize;
    while let Some(lt_rel) = svg[i..].find('<') {
        let lt = i + lt_rel;
        out.push_str(&svg[i..lt]);

        let Some(gt_rel) = svg[lt..].find('>') else {
            out.push_str(&svg[lt..]);
            i = svg.len();
            break;
        };
        let gt = lt + gt_rel + 1;
        let tag = &svg[lt..gt];

        // Comments / declarations: passthrough.
        if tag.starts_with("<!--") || tag.starts_with("<!") || tag.starts_with("<?") {
            out.push_str(tag);
            i = gt;
            continue;
        }

        if tag.starts_with("</g") {
            let _ = g_stack.pop();
            out.push_str(tag);
            i = gt;
            continue;
        }

        if tag.starts_with("<g") {
            if !is_self_closing(tag) {
                g_stack.push(GFrame::from_g_tag(tag));
            }
            out.push_str(tag);
            i = gt;
            continue;
        }

        if tag.starts_with("<foreignObject") {
            let start_end = gt;
            let Some(close_rel) = svg[start_end..].find(close_tag) else {
                out.push_str(&svg[lt..]);
                i = svg.len();
                break;
            };
            let inner_start = start_end;
            let inner_end = inner_start + close_rel;
            let inner = &svg[inner_start..inner_end];
            let i_next = inner_end + close_tag.len();

            out.push_str(&svg[lt..i_next]);

            let width = parse_attr_f64(tag, "width").unwrap_or(0.0);
            let height = parse_attr_f64(tag, "height").unwrap_or(0.0);
            if width > 0.0 && height > 0.0 {
                let x = parse_attr_f64(tag, "x").unwrap_or(0.0);
                let y = parse_attr_f64(tag, "y").unwrap_or(0.0);
                let base = sum_translate(&g_stack);

                let abs_x = base.x + x;
                let abs_y = base.y + y;
                let (anchor, text_x) = match parse_attr_str(tag, "text-anchor") {
                    Some("start") => ("start", abs_x),
                    Some("end") => ("end", abs_x + width),
                    _ => ("middle", abs_x + width / 2.0),
                };
                let text_y = abs_y + height / 2.0;

                let raw_lines = htmlish_to_text_lines(inner);
                if !raw_lines.is_empty() {
                    overlays.push_str(&format!(
                        r#"<g data-merman-foreignobject="fallback" class="{}">"#,
                        class_attr_tokens(&g_stack, inner, "merman-foreignobject-fallback")
                    ));

                    let wants_label_bkg = inner.contains("labelBkg");
                    if wants_label_bkg {
                        overlays.push_str(&format!(
                            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                            abs_x,
                            abs_y,
                            width,
                            height,
                            escape_xml_attr(&label_bkg)
                        ));
                    }

                    let font_size_value = extract_inline_html_style_property(inner, "font-size")
                        .or_else(|| extract_svg_font_style_from_context(svg, &g_stack, "font-size"))
                        .unwrap_or_else(|| "16px".to_string());
                    let font_size = parse_css_px(&font_size_value, 16.0);
                    let fill = extract_inline_html_color(inner)
                        .or_else(|| extract_svg_text_fill_from_ancestors(svg, &g_stack))
                        .unwrap_or_else(|| "#333".to_string());
                    let font_family = extract_inline_html_style_property(inner, "font-family")
                        .or_else(|| {
                            extract_svg_font_style_from_context(svg, &g_stack, "font-family")
                        })
                        .unwrap_or_else(|| "trebuchet ms,verdana,arial,sans-serif".to_string());
                    let font_weight = extract_inline_html_style_property(inner, "font-weight")
                        .or_else(|| {
                            extract_svg_font_style_from_context(svg, &g_stack, "font-weight")
                        });
                    let font_style = extract_inline_html_style_property(inner, "font-style")
                        .or_else(|| {
                            extract_svg_font_style_from_context(svg, &g_stack, "font-style")
                        });
                    let measure_style = TextStyle {
                        font_family: Some(font_family.clone()),
                        font_size,
                        font_weight: font_weight.clone(),
                    };
                    let wrap_width = foreign_object_html_soft_wrap_width(tag, inner);
                    let lines = wrap_html_lines_to_width(
                        raw_lines,
                        wrap_width,
                        &fallback_measurer,
                        &measure_style,
                    );
                    let line_height = font_size * 1.5;
                    let n = lines.len() as f64;
                    let y0 = text_y - (line_height * (n - 1.0)) / 2.0;
                    let mut text_style = format!(
                        "text-anchor: {anchor}; font-size: {font_size_value}; font-family: {font_family};"
                    );
                    if let Some(font_weight) = font_weight {
                        text_style.push_str(" font-weight: ");
                        text_style.push_str(&font_weight);
                        text_style.push(';');
                    }
                    if let Some(font_style) = font_style {
                        text_style.push_str(" font-style: ");
                        text_style.push_str(&font_style);
                        text_style.push(';');
                    }
                    let text_class = fallback_text_class_attr_tokens(&g_stack, inner);

                    for (idx, line) in lines.iter().enumerate() {
                        let y_line = y0 + (idx as f64) * line_height;
                        let text = escape_xml_text(line);
                        overlays.push_str(&format!(
                            r##"<text x="{}" y="{}" dominant-baseline="central" alignment-baseline="central" fill="{}" class="{}" style="{}">{}</text>"##,
                            text_x,
                            y_line,
                            escape_xml_attr(&fill),
                            text_class,
                            escape_xml_attr(&text_style),
                            text
                        ));
                    }

                    overlays.push_str("</g>");
                }
            }

            i = i_next;
            continue;
        }

        out.push_str(tag);
        i = gt;
    }

    if i < svg.len() {
        out.push_str(&svg[i..]);
    }

    if overlays.is_empty() {
        return out;
    }

    if let Some(idx) = out.rfind("</svg>") {
        let mut with_overlays = String::with_capacity(out.len() + overlays.len() + 64);
        with_overlays.push_str(&out[..idx]);
        with_overlays.push_str(&overlays);
        with_overlays.push_str(&out[idx..]);
        with_overlays
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::foreign_object_label_fallback_svg_text;

    #[test]
    fn foreign_object_overlay_accounts_for_parent_translate() {
        let svg = r#"<svg viewBox="90 -310 425 99" xmlns="http://www.w3.org/2000/svg"><g transform="translate(183.3046875, -300)"><foreignObject width="33.390625" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Todo</p></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);
        assert!(
            out.contains(r#"x="200""#),
            "expected x=200 center placement"
        );
        assert!(
            out.contains(r#"y="-288""#),
            "expected y=-288 center placement"
        );
        assert!(
            out.contains(">Todo<"),
            "expected text content to be present"
        );
    }

    #[test]
    fn foreign_object_overlay_renders_label_bkg_rect_when_present() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>#d .labelBkg{background-color:rgba(232,232,232,0.5);}</style><g id="d"><foreignObject x="10" y="20" width="30" height="24"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg"><p>Hello</p></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);
        assert!(
            out.contains(r#"fill="rgba(232,232,232,0.5)""#),
            "expected labelBkg fill"
        );
        assert!(
            out.contains(r#"<rect x="10" y="20" width="30" height="24""#),
            "expected rect with foreignObject bounds"
        );
    }

    #[test]
    fn foreign_object_overlay_splits_literal_backslash_n() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="translate(10, 20)"><foreignObject width="80" height="48"><div xmlns="http://www.w3.org/1999/xhtml"><p>Layer 7\nHTTP</p></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);
        assert!(out.contains(">Layer 7<"), "got: {out}");
        assert!(out.contains(">HTTP<"), "got: {out}");
        assert!(
            !out.contains(">Layer 7\\nHTTP</text>"),
            "literal backslash-n should not remain in fallback text overlay: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_propagates_style_context() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><g class="node selected" fill="#112233" style="font-size: 14px; font-family: Inter; font-weight: 600;"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg host-label" style="color: #abcdef; font-style: italic;"><p>Hello</p></div></foreignObject></g></svg>"##;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains(
                r#"class="merman-foreignobject-fallback node selected labelBkg host-label""#
            ),
            "expected fallback group to keep host-relevant classes: {out}"
        );
        assert!(
            out.contains(
                r#"class="merman-foreignobject-fallback-text node selected labelBkg host-label""#
            ),
            "expected fallback text to keep host-relevant classes: {out}"
        );
        assert!(
            out.contains(r##"fill="#abcdef""##),
            "expected inline HTML color to drive fallback fill: {out}"
        );
        assert!(
            out.contains("font-size: 14px")
                && out.contains("font-family: Inter")
                && out.contains("font-weight: 600")
                && out.contains("font-style: italic"),
            "expected font context to propagate: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_uses_scoped_label_css_for_fallback_fill() {
        let svg = r##"<svg id="host-theme-block" xmlns="http://www.w3.org/2000/svg"><style>#host-theme-block{fill:#eeeeee;}#host-theme-block .node rect{fill:#111827;}#host-theme-block .label text,#host-theme-block span,#host-theme-block p{fill:#e5e7eb;color:#e5e7eb;}</style><g class="block"><g class="node flowchart-label"><g class="label"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Alpha</p></div></foreignObject></g></g></g></svg>"##;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains(r##"fill="#e5e7eb""##),
            "expected scoped label CSS, not shape CSS/default fill, to drive fallback text: {out}"
        );
        assert!(
            !out.contains(r##"fill="#111827""##),
            "fallback text should not inherit node rectangle fill: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_uses_root_fill_when_no_label_context_matches() {
        let svg = r##"<svg id="host-theme-root" xmlns="http://www.w3.org/2000/svg"><style>#host-theme-root{font-family:Inter;fill:#ddeeff;}#host-theme-root .node rect{fill:#111827;}</style><g><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Alpha</p></div></foreignObject></g></svg>"##;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains(r##"fill="#ddeeff""##),
            "expected root fill to be the final readable fallback: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_uses_root_font_context() {
        let svg = r##"<svg id="host-theme-root-font" xmlns="http://www.w3.org/2000/svg"><style>#host-theme-root-font{font-family:Inter,system-ui;font-size:14px;fill:#ddeeff;}</style><g><foreignObject width="80" height="21"><div xmlns="http://www.w3.org/1999/xhtml"><p>Alpha</p></div></foreignObject></g></svg>"##;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(out.contains(r##"fill="#ddeeff""##), "got: {out}");
        assert!(out.contains("font-size: 14px"), "got: {out}");
        assert!(out.contains("font-family: Inter,system-ui"), "got: {out}");
    }

    #[test]
    fn foreign_object_overlay_does_not_put_structural_label_class_on_text() {
        let svg = r##"<svg id="host-theme-edge-label" xmlns="http://www.w3.org/2000/svg"><style>#host-theme-edge-label .edgeLabel .label{fill:#665c54;font-size:14px;}#host-theme-edge-label .edgeLabel .label text{fill:#ebdbb2;}</style><g class="edgeLabel"><g class="label"><foreignObject width="80" height="21"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg"><span class="edgeLabel">places</span></div></foreignObject></g></g></svg>"##;
        let out = foreign_object_label_fallback_svg_text(svg);
        let text_tag_start = out
            .find(r#"<text "#)
            .unwrap_or_else(|| panic!("expected fallback text: {out}"));
        let text_tag_end = out[text_tag_start..]
            .find('>')
            .map(|offset| text_tag_start + offset)
            .unwrap_or_else(|| panic!("expected fallback text tag end: {out}"));
        let text_tag = &out[text_tag_start..=text_tag_end];

        assert!(text_tag.contains(r##"fill="#ebdbb2""##), "got: {out}");
        assert!(
            !text_tag.contains(r#"class="merman-foreignobject-fallback-text edgeLabel label "#)
                && !text_tag.contains(r#" label""#),
            "fallback text should not keep the structural label class: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_decodes_double_escaped_html_entities_for_fallback_text() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g><foreignObject width="120" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>List&amp;lt;Animal&amp;gt; &amp;amp; friends</p></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains(">List&lt;Animal&gt; &amp; friends<"),
            "expected fallback text to avoid double-escaped entities: {out}"
        );
        let fallback = &out[out
            .find(r#"data-merman-foreignobject="fallback""#)
            .expect("fallback group")..];
        assert!(!fallback.contains("&amp;lt;"), "got: {fallback}");
        assert!(!fallback.contains("&amp;gt;"), "got: {fallback}");
    }

    #[test]
    fn foreign_object_overlay_wraps_break_spaces_labels_to_foreign_object_width() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="translate(20, 30)"><foreignObject width="200" height="48"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table; white-space: break-spaces; line-height: 1.5; max-width: 200px; text-align: center; width: 200px;"><span class="nodeLabel">Import / WebSurface / Data Egress Gates</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            !out.contains(">Import / WebSurface / Data Egress Gates</text>"),
            "fallback text should inherit Mermaid HTML soft wrapping instead of flattening into one SVG text line: {out}"
        );
        assert!(
            out.contains(">Import / WebSurface /<") && out.contains(">Data Egress Gates<"),
            "expected fallback text to wrap into readable SVG lines: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_keeps_nowrap_labels_as_single_fallback_line() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g><foreignObject width="200" height="24"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel">Import / WebSurface / Data Egress Gates</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains(">Import / WebSurface / Data Egress Gates</text>"),
            "explicit nowrap labels should keep the existing single-line fallback behavior: {out}"
        );
    }
}
