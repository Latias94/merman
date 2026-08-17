#![forbid(unsafe_code)]

// NOTE: This fallback module intentionally keeps parsing "cheap" and non-validating.
// It is a best-effort readability fallback for SVG consumers that do not fully
// support HTML inside `<foreignObject>` (e.g. many rasterizers).

mod attr;
mod context;
mod css;
mod html;

use crate::svg::pipeline::{
    checkpoint_loop, escape_xml_attr_with_checkpoints, escape_xml_text_with_checkpoints,
    extract_exact_double_quoted_attr_with_checkpoints, find_tag_end_with_checkpoints,
    find_with_checkpoints, rfind_with_checkpoints,
};
use crate::text::{TextMeasurer, TextStyle};
use std::convert::Infallible;

use attr::is_self_closing;
use context::{
    GFrame, class_attr_tokens, extract_svg_font_style_from_context,
    extract_svg_text_fill_from_ancestors, fallback_text_class_attr_tokens, sum_translate,
};
use css::FallbackStyleIndex;
use html::{
    extract_inline_html_color, extract_inline_html_style_property,
    foreign_object_html_soft_wrap_width, htmlish_to_text_lines, parse_css_px,
    wrap_html_lines_to_width,
};

/// Adds a best-effort `<text>/<tspan>` overlay extracted from Mermaid label `<foreignObject>`
/// content.
///
/// Many headless SVG renderers and rasterizers do not fully support HTML inside `<foreignObject>`.
/// The returned SVG aims to be *more readable* for raster outputs and UI previews.
///
/// Important:
/// - This does not aim for Mermaid DOM parity.
/// - For parity-focused SVG output, keep the original SVG unchanged.
pub fn foreign_object_label_fallback_svg_text(
    svg: &str,
    text_measurer: &dyn TextMeasurer,
) -> String {
    let mut checkpoint = || Ok::<(), Infallible>(());
    match foreign_object_label_fallback_svg_text_with_checkpoints(
        svg,
        text_measurer,
        &mut checkpoint,
    ) {
        Ok(output) => output,
        Err(error) => match error {},
    }
}

/// Controlled variant used by the SVG postprocess pipeline.
///
/// The public helper above remains infallible for compatibility. Pipeline callers provide the
/// operation-owned Postprocess checkpoint so a host measurement that declines, fails, or returns
/// an invalid value cannot enter its fallback backend after cancellation.
pub(crate) fn foreign_object_label_fallback_svg_text_controlled(
    svg: &str,
    text_measurer: &dyn TextMeasurer,
    mut checkpoint: impl FnMut() -> crate::Result<()>,
) -> crate::Result<String> {
    foreign_object_label_fallback_svg_text_with_checkpoints(svg, text_measurer, &mut checkpoint)
}

fn parse_attr_f64<E>(
    tag: &str,
    name: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<f64>, E> {
    let Some(value) = extract_exact_double_quoted_attr_with_checkpoints(tag, name, checkpoint)?
    else {
        return Ok(None);
    };
    checkpoint()?;
    let value = value.parse::<f64>().ok();
    checkpoint()?;
    Ok(value)
}

fn foreign_object_label_fallback_svg_text_with_checkpoints<E>(
    svg: &str,
    text_measurer: &dyn TextMeasurer,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    checkpoint()?;
    if find_with_checkpoints(svg, "<foreignObject", checkpoint)?.is_none() {
        return Ok(svg.to_string());
    }

    let close_tag = "</foreignObject>";
    let mut out = String::with_capacity(svg.len() + 2048);
    let mut overlays = String::new();
    let mut g_stack: Vec<GFrame> = Vec::new();
    let style_index = FallbackStyleIndex::new(svg, checkpoint)?;
    let label_bkg_default = "rgba(232, 232, 232, 0.5)".to_string();
    let label_bkg = style_index
        .background_color_for_class("labelBkg")
        .map(str::to_owned)
        .unwrap_or(label_bkg_default);
    let mut i = 0usize;
    let mut iteration = 0usize;
    while let Some(lt_rel) = find_with_checkpoints(&svg[i..], "<", checkpoint)? {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        let lt = i + lt_rel;
        out.push_str(&svg[i..lt]);

        let Some(gt) = find_tag_end_with_checkpoints(svg, lt, checkpoint)? else {
            out.push_str(&svg[lt..]);
            i = svg.len();
            break;
        };
        let gt = gt + 1;
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
                g_stack.push(GFrame::from_g_tag(tag, checkpoint)?);
            }
            out.push_str(tag);
            i = gt;
            continue;
        }

        if tag.starts_with("<foreignObject") {
            let start_end = gt;
            let Some(close_rel) = find_with_checkpoints(&svg[start_end..], close_tag, checkpoint)?
            else {
                out.push_str(&svg[lt..]);
                i = svg.len();
                break;
            };
            let inner_start = start_end;
            let inner_end = inner_start + close_rel;
            let inner = &svg[inner_start..inner_end];
            let i_next = inner_end + close_tag.len();

            out.push_str(&svg[lt..i_next]);

            let from_switch =
                is_foreign_object_switch_native_fallback(svg, lt, i_next, checkpoint)?;

            let width = parse_attr_f64(tag, "width", checkpoint)?.unwrap_or(0.0);
            let height = parse_attr_f64(tag, "height", checkpoint)?.unwrap_or(0.0);
            if width > 0.0 && height > 0.0 {
                let x = parse_attr_f64(tag, "x", checkpoint)?.unwrap_or(0.0);
                let y = parse_attr_f64(tag, "y", checkpoint)?.unwrap_or(0.0);
                let base = sum_translate(&g_stack, checkpoint)?;

                let abs_x = base.x + x;
                let abs_y = base.y + y;
                let (anchor, text_x) = match extract_exact_double_quoted_attr_with_checkpoints(
                    tag,
                    "text-anchor",
                    checkpoint,
                )? {
                    Some("start") => ("start", abs_x),
                    Some("end") => ("end", abs_x + width),
                    _ => ("middle", abs_x + width / 2.0),
                };
                let text_y = abs_y + height / 2.0;

                checkpoint()?;
                let raw_lines = htmlish_to_text_lines(inner, checkpoint)?;
                if !raw_lines.is_empty() {
                    let source_attr = if from_switch {
                        r#" data-merman-foreignobject-source="switch-native-fallback""#
                    } else {
                        ""
                    };
                    overlays.push_str(&format!(
                        r#"<g data-merman-foreignobject="fallback"{source} class="{cls}">"#,
                        source = source_attr,
                        cls = class_attr_tokens(
                            &g_stack,
                            inner,
                            "merman-foreignobject-fallback",
                            checkpoint,
                        )?
                    ));

                    let wants_label_bkg =
                        find_with_checkpoints(inner, "labelBkg", checkpoint)?.is_some();
                    if wants_label_bkg {
                        overlays.push_str(&format!(
                            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                            abs_x,
                            abs_y,
                            width,
                            height,
                            escape_xml_attr_with_checkpoints(&label_bkg, checkpoint)?
                        ));
                    }

                    let font_size_value =
                        match extract_inline_html_style_property(inner, "font-size", checkpoint)? {
                            Some(value) => value,
                            None => extract_svg_font_style_from_context(
                                &style_index,
                                &g_stack,
                                "font-size",
                                checkpoint,
                            )?
                            .unwrap_or_else(|| "16px".to_string()),
                        };
                    let font_size = parse_css_px(&font_size_value, 16.0);
                    let fill = match extract_inline_html_color(inner, checkpoint)? {
                        Some(value) => value,
                        None => extract_svg_text_fill_from_ancestors(
                            &style_index,
                            &g_stack,
                            checkpoint,
                        )?
                        .unwrap_or_else(|| "#333".to_string()),
                    };
                    let font_family =
                        match extract_inline_html_style_property(inner, "font-family", checkpoint)?
                        {
                            Some(value) => value,
                            None => extract_svg_font_style_from_context(
                                &style_index,
                                &g_stack,
                                "font-family",
                                checkpoint,
                            )?
                            .unwrap_or_else(|| "trebuchet ms,verdana,arial,sans-serif".to_string()),
                        };
                    let font_weight =
                        match extract_inline_html_style_property(inner, "font-weight", checkpoint)?
                        {
                            Some(value) => Some(value),
                            None => extract_svg_font_style_from_context(
                                &style_index,
                                &g_stack,
                                "font-weight",
                                checkpoint,
                            )?,
                        };
                    let font_style = match extract_inline_html_style_property(
                        inner,
                        "font-style",
                        checkpoint,
                    )? {
                        Some(value) => Some(value),
                        None => extract_svg_font_style_from_context(
                            &style_index,
                            &g_stack,
                            "font-style",
                            checkpoint,
                        )?,
                    };
                    let measure_style = TextStyle {
                        font_family: Some(font_family.clone()),
                        font_size,
                        font_weight: font_weight.clone(),
                        font_style: None,
                    };
                    let wrap_width = foreign_object_html_soft_wrap_width(tag, inner, checkpoint)?;
                    let lines = wrap_html_lines_to_width(
                        raw_lines,
                        wrap_width,
                        text_measurer,
                        &measure_style,
                        checkpoint,
                    )?;
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
                    let text_class = fallback_text_class_attr_tokens(&g_stack, inner, checkpoint)?;

                    for (idx, line) in lines.iter().enumerate() {
                        checkpoint_loop(idx, checkpoint)?;
                        let y_line = y0 + (idx as f64) * line_height;
                        let text = escape_xml_text_with_checkpoints(line, checkpoint)?;
                        overlays.push_str(&format!(
                            r##"<text x="{}" y="{}" dominant-baseline="central" alignment-baseline="central" fill="{}" class="{}" style="{}">{}</text>"##,
                            text_x,
                            y_line,
                            escape_xml_attr_with_checkpoints(&fill, checkpoint)?,
                            text_class,
                            escape_xml_attr_with_checkpoints(&text_style, checkpoint)?,
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
        checkpoint()?;
        return Ok(out);
    }

    let result = if let Some(idx) = rfind_with_checkpoints(&out, "</svg>", checkpoint)? {
        let mut with_overlays = String::with_capacity(out.len() + overlays.len() + 64);
        with_overlays.push_str(&out[..idx]);
        with_overlays.push_str(&overlays);
        with_overlays.push_str(&out[idx..]);
        with_overlays
    } else {
        out
    };
    checkpoint()?;
    Ok(result)
}

fn is_foreign_object_switch_native_fallback<E>(
    svg: &str,
    foreign_object_start: usize,
    foreign_object_end: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    let Some(switch_start) = find_wrapping_switch_start(svg, foreign_object_start, checkpoint)?
    else {
        return Ok(false);
    };
    let Some(switch_close_rel) =
        find_with_checkpoints(&svg[foreign_object_end..], "</switch>", checkpoint)?
    else {
        return Ok(false);
    };

    let has_intervening_close = find_with_checkpoints(
        &svg[switch_start..foreign_object_start],
        "</switch>",
        checkpoint,
    )?
    .is_some();
    let has_native_text = find_with_checkpoints(
        &svg[foreign_object_end..foreign_object_end + switch_close_rel],
        "<text",
        checkpoint,
    )?
    .is_some();
    Ok(!has_intervening_close && has_native_text)
}

fn find_wrapping_switch_start<E>(
    svg: &str,
    before: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    let mut search_end = before;
    while search_end > 0 {
        let Some(start) = rfind_with_checkpoints(&svg[..search_end], "<switch", checkpoint)? else {
            return Ok(None);
        };
        let Some(open_end) = find_tag_end_with_checkpoints(svg, start, checkpoint)? else {
            return Ok(None);
        };
        if open_end >= before {
            search_end = start;
            continue;
        }

        let tag = &svg[start..=open_end];
        if is_start_switch_tag(tag) {
            return Ok(Some(start));
        }

        search_end = start;
    }
    Ok(None)
}

fn is_start_switch_tag(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    tag.starts_with("<switch")
        && bytes
            .get("<switch".len())
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/')
        && !tag.trim_end().ends_with("/>")
}

#[cfg(test)]
mod tests {
    use super::foreign_object_label_fallback_svg_text as render_fallback;
    use crate::text::VendoredFontMetricsTextMeasurer;

    fn foreign_object_label_fallback_svg_text(svg: &str) -> String {
        render_fallback(svg, &VendoredFontMetricsTextMeasurer::default())
    }

    #[test]
    fn foreign_object_inside_switch_with_native_text_generates_tagged_fallback() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><switch><foreignObject x="150" y="50" width="550" height="50"><div class="journey-section" xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><div class="label" style="display: table-cell; text-align: center; vertical-align: middle;">Go to work</div></div></foreignObject><text x="425" y="75" fill="#333"><tspan x="425" dy="0">Go to work</tspan></text></switch></svg>"##;
        let out = foreign_object_label_fallback_svg_text(svg);
        assert!(
            out.contains(r#"data-merman-foreignobject="fallback""#),
            "should generate fallback overlay: {out}"
        );
        assert!(
            out.contains(r#"data-merman-foreignobject-source="switch-native-fallback""#),
            "fallback should be tagged with switch source: {out}"
        );
    }

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
    fn foreign_object_overlay_ignores_non_stylesheet_markup_before_and_inside_style() {
        let svg = r##"<svg id="brace-theme" xmlns="http://www.w3.org/2000/svg"><!-- > <style>#brace-theme{fill:red;}</style> --><text>{</text><style><![CDATA[/* </style> */ #brace-theme{fill:#ddeeff;font-family:Inter;}#brace-theme .labelBkg{background-color:#c0ffee;}]]></style><g><foreignObject x="10" y="20" width="30" height="24"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg"><p>Alpha</p></div></foreignObject></g></svg>"##;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains(r##"fill="#ddeeff""##),
            "expected the real root rule after authored brace text to drive fallback text: {out}"
        );
        assert!(
            out.contains(r##"fill="#c0ffee""##)
                && out.contains(r#"<rect x="10" y="20" width="30" height="24""#),
            "expected the real labelBkg rule after authored brace text to drive fallback background: {out}"
        );
        assert!(
            out.contains("font-family: Inter"),
            "expected root font declarations to remain indexed: {out}"
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
    fn foreign_object_overlay_reads_unicode_root_style_identity() {
        let svg = r##"<svg id="图表-α" xmlns="http://www.w3.org/2000/svg"><style>#图表-α{font-family:Inter;font-size:14px;fill:#ddeeff;}</style><g><foreignObject width="80" height="21"><div xmlns="http://www.w3.org/1999/xhtml"><p>Alpha</p></div></foreignObject></g></svg>"##;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(out.contains(r##"fill="#ddeeff""##), "got: {out}");
        assert!(out.contains("font-family: Inter"), "got: {out}");
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
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g><foreignObject width="220" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>List&amp;lt;Animal&amp;gt; &amp;amp; friends &amp;apos;x&amp;apos; &amp;quot;y&amp;quot; &amp;#39;z&amp;#39; &amp;#x2F;</p></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains(">List&lt;Animal&gt; &amp; friends 'x' \"y\" 'z' /<"),
            "expected fallback text to avoid double-escaped entities: {out}"
        );
        let fallback = &out[out
            .find(r#"data-merman-foreignobject="fallback""#)
            .expect("fallback group")..];
        assert!(!fallback.contains("&amp;lt;"), "got: {fallback}");
        assert!(!fallback.contains("&amp;gt;"), "got: {fallback}");
        assert!(!fallback.contains("&amp;amp;"), "got: {fallback}");
        assert!(!fallback.contains("&amp;apos;"), "got: {fallback}");
        assert!(!fallback.contains("&amp;quot;"), "got: {fallback}");
        assert!(!fallback.contains("&amp;#"), "got: {fallback}");
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
