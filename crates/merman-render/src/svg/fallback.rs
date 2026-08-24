#![forbid(unsafe_code)]

// NOTE: This fallback module intentionally keeps parsing "cheap" and non-validating.
// It is a best-effort readability fallback for SVG consumers that do not fully
// support HTML inside `<foreignObject>` (e.g. many rasterizers).

mod attr;
mod cascade;
mod context;
mod css;
mod html;

use crate::svg::pipeline::{
    SvgPostprocessExecution, SvgStructureMetrics, SvgTagScanner, checkpoint_loop, end_tag_name,
    escape_xml_attr_with_checkpoints, escape_xml_text_with_checkpoints,
    extract_exact_double_quoted_attr_with_checkpoints, find_tag_end_with_checkpoints,
    find_with_checkpoints, rfind_with_checkpoints, start_tag_name,
};
use crate::text::TextMeasurer;
use std::convert::Infallible;
use std::fmt::{self, Write};

use attr::is_self_closing;
use cascade::{CascadeIndex, Namespace, SourceElement};
use context::{GFrame, source_class_attr_tokens, sum_translate};
use html::{foreign_object_html_soft_wrap_width, htmlish_to_text_lines, wrap_html_lines_to_width};

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
        &mut |_, _| Ok::<(), Infallible>(()),
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
    execution: SvgPostprocessExecution<'_>,
    structure: SvgStructureMetrics,
) -> crate::Result<String> {
    foreign_object_label_fallback_svg_text_with_checkpoints(
        svg,
        text_measurer,
        &mut || execution.checkpoint(),
        &mut |projected_bytes, generated_elements| {
            execution.preflight_svg_byte_count(projected_bytes)?;
            execution.preflight_svg_structure(
                structure.elements.saturating_add(generated_elements),
                structure.max_tree_depth.max(2),
            )
        },
    )
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
    preflight_generated: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<String, E> {
    checkpoint()?;
    if find_with_checkpoints(svg, "<foreignObject", checkpoint)?.is_none() {
        return Ok(svg.to_string());
    }

    let close_tag = "</foreignObject>";
    preflight_generated(svg.len(), 0)?;
    let mut out = String::with_capacity(svg.len());
    let mut overlays = String::new();
    let mut generated_elements = 0usize;
    let mut g_stack: Vec<GFrame> = Vec::new();
    let mut source_stack: Vec<SourceElement> = Vec::new();
    let mut cascade_index = None;
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

        if let Some(name) = end_tag_name(tag) {
            if name.eq_ignore_ascii_case("g") {
                let _ = g_stack.pop();
            }
            if source_stack
                .last()
                .is_some_and(|element| element.local_name.eq_ignore_ascii_case(name))
            {
                source_stack.pop();
            }
            out.push_str(tag);
            i = gt;
            continue;
        }

        let start_name = start_tag_name(tag);
        if start_name.is_some_and(|name| name.eq_ignore_ascii_case("foreignObject")) {
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
                    let cascade = match cascade_index.as_ref() {
                        Some(cascade) => cascade,
                        None => {
                            cascade_index = Some(CascadeIndex::new(svg, checkpoint)?);
                            cascade_index
                                .as_ref()
                                .expect("cascade index was initialized")
                        }
                    };
                    let typography =
                        cascade.resolve_foreign_object(&source_stack, tag, inner, checkpoint)?;
                    let source_attr = if from_switch {
                        r#" data-merman-foreignobject-source="switch-native-fallback""#
                    } else {
                        ""
                    };
                    let source_classes = source_class_attr_tokens(&g_stack, inner, checkpoint)?;
                    let source_classes_attr = source_classes
                        .as_deref()
                        .map(|classes| format!(r#" data-merman-source-classes="{classes}""#))
                        .unwrap_or_default();
                    push_generated_fmt(
                        &mut overlays,
                        format_args!(
                            r#"<g data-merman-foreignobject="fallback"{source} class="merman-foreignobject-fallback"{source_classes}>"#,
                            source = source_attr,
                            source_classes = source_classes_attr,
                        ),
                        1,
                        svg.len(),
                        &mut generated_elements,
                        preflight_generated,
                    )?;

                    if let Some(label_bkg) = &typography.label_background {
                        let escaped_label_bkg =
                            escape_xml_attr_with_checkpoints(label_bkg, checkpoint)?;
                        push_generated_fmt(
                            &mut overlays,
                            format_args!(
                                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                                abs_x, abs_y, width, height, escaped_label_bkg,
                            ),
                            1,
                            svg.len(),
                            &mut generated_elements,
                            preflight_generated,
                        )?;
                    }

                    let measure_style = typography.text_style();
                    let wrap_width = foreign_object_html_soft_wrap_width(tag, inner, checkpoint)?;
                    let lines = wrap_html_lines_to_width(
                        raw_lines,
                        wrap_width,
                        text_measurer,
                        &measure_style,
                        checkpoint,
                    )?;
                    let line_height = typography.line_height;
                    let n = lines.len() as f64;
                    let y0 = text_y - (line_height * (n - 1.0)) / 2.0;
                    let mut text_style = format!(
                        "text-anchor: {anchor}; font-size: {}px; font-family: {}; line-height: {}px;",
                        typography.font_size, typography.font_family, typography.line_height,
                    );
                    if let Some(font_weight) = &typography.font_weight {
                        text_style.push_str(" font-weight: ");
                        text_style.push_str(font_weight);
                        text_style.push(';');
                    }
                    if let Some(font_style) = &typography.font_style {
                        text_style.push_str(" font-style: ");
                        text_style.push_str(font_style);
                        text_style.push(';');
                    }
                    let escaped_fill =
                        escape_xml_attr_with_checkpoints(&typography.fill, checkpoint)?;
                    let escaped_style = escape_xml_attr_with_checkpoints(&text_style, checkpoint)?;

                    for (idx, line) in lines.iter().enumerate() {
                        checkpoint_loop(idx, checkpoint)?;
                        let y_line = y0 + (idx as f64) * line_height;
                        let text = escape_xml_text_with_checkpoints(line, checkpoint)?;
                        push_generated_fmt(
                            &mut overlays,
                            format_args!(
                                r##"<text x="{}" y="{}" dominant-baseline="central" alignment-baseline="central" fill="{}" class="merman-foreignobject-fallback-text"{} style="{}">{}</text>"##,
                                text_x,
                                y_line,
                                escaped_fill,
                                source_classes_attr,
                                escaped_style,
                                text
                            ),
                            1,
                            svg.len(),
                            &mut generated_elements,
                            preflight_generated,
                        )?;
                    }

                    push_generated_fmt(
                        &mut overlays,
                        format_args!("</g>"),
                        0,
                        svg.len(),
                        &mut generated_elements,
                        preflight_generated,
                    )?;
                }
            }
            i = i_next;
            continue;
        }

        // Source styles are consumed by `CascadeIndex` in its first pass. Do
        // not treat CSS text (or CDATA containing a literal `</style>`) as SVG
        // structure while building the source ancestry for conversion.
        if start_name.is_some_and(|name| name.eq_ignore_ascii_case("style"))
            && !is_self_closing(tag)
        {
            let mut scanner = SvgTagScanner::new(svg);
            scanner.skip_to(gt);
            let mut close_end = None;
            while let Some(candidate) = scanner.next_with_checkpoints(checkpoint)? {
                if end_tag_name(candidate.raw())
                    .is_some_and(|name| name.eq_ignore_ascii_case("style"))
                {
                    close_end = Some(scanner.cursor());
                    break;
                }
            }
            if let Some(close_end) = close_end {
                out.push_str(&svg[lt..close_end]);
                i = close_end;
                continue;
            }
        }

        if let Some(name) = start_name {
            if name.eq_ignore_ascii_case("g") && !is_self_closing(tag) {
                g_stack.push(GFrame::from_g_tag(tag, checkpoint)?);
            }
            if !is_self_closing(tag) {
                source_stack.push(CascadeIndex::source_element(
                    tag,
                    Namespace::Svg,
                    checkpoint,
                )?);
            }
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
        let mut with_overlays = String::with_capacity(out.len() + overlays.len());
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

#[derive(Default)]
struct FmtByteCounter {
    bytes: usize,
}

impl Write for FmtByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.saturating_add(value.len());
        Ok(())
    }
}

fn push_generated_fmt<E>(
    output: &mut String,
    arguments: fmt::Arguments<'_>,
    added_elements: usize,
    source_bytes: usize,
    generated_elements: &mut usize,
    preflight_generated: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<(), E> {
    let mut counter = FmtByteCounter::default();
    counter
        .write_fmt(arguments)
        .expect("counting formatted SVG bytes cannot fail");
    let projected_bytes = source_bytes
        .saturating_add(output.len())
        .saturating_add(counter.bytes);
    let projected_elements = generated_elements.saturating_add(added_elements);
    preflight_generated(projected_bytes, projected_elements)?;
    output.reserve(counter.bytes);
    output
        .write_fmt(arguments)
        .expect("writing formatted SVG into a String cannot fail");
    *generated_elements = projected_elements;
    Ok(())
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
    use crate::text::{TextMeasurer, TextMetrics, TextStyle, VendoredFontMetricsTextMeasurer};
    use std::cell::RefCell;

    #[derive(Default)]
    struct RecordingMeasurer {
        styles: RefCell<Vec<TextStyle>>,
    }

    impl TextMeasurer for RecordingMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.styles.borrow_mut().push(style.clone());
            TextMetrics {
                width: text.chars().count() as f64 * style.font_size,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

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
    fn foreign_object_overlay_does_not_inherit_background_color_by_default() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g style="background-color:#ff0000"><foreignObject x="10" y="20" width="30" height="24"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg"><p>Hello</p></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);
        assert!(
            !out.contains(r##"fill="#ff0000""##),
            "background-color is not inherited by a child without a specified value: {out}"
        );

        let explicit_initial = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.labelBkg{background-color:initial}</style><foreignObject x="10" y="20" width="30" height="24"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg"><p>Hello</p></div></foreignObject></svg>"#;
        let out = foreign_object_label_fallback_svg_text(explicit_initial);
        let fallback = out
            .split(r#"data-merman-foreignobject="fallback""#)
            .nth(1)
            .unwrap_or_else(|| panic!("expected fallback output: {out}"));
        assert!(
            !fallback.contains("rgba(232, 232, 232, 0.5)"),
            "explicit background-color: initial must not become the compatibility gray: {out}"
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
                r#"class="merman-foreignobject-fallback" data-merman-source-classes="node selected labelBkg host-label""#
            ),
            "expected fallback group to retain source classes as inert metadata: {out}"
        );
        assert!(
            out.contains(
                r#"class="merman-foreignobject-fallback-text" data-merman-source-classes="node selected labelBkg host-label""#
            ),
            "expected fallback text to retain source classes as inert metadata: {out}"
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
    fn foreign_object_overlay_does_not_flatten_descendant_font_selector() {
        let svg = r#"<svg id="class-context" xmlns="http://www.w3.org/2000/svg"><style>#class-context{font-size:16px}.classLabel .label{font-size:10px}</style><g class="node"><g class="label"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains("font-size: 16px"),
            "a missing .classLabel ancestor must not collapse the selector to .label: {out}"
        );
        assert!(
            !out.contains("font-size: 10px"),
            "the contextual 10px rule must not leak into the class-node label: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_matches_real_descendant_font_selector() {
        let svg = r#"<svg id="class-context-positive" xmlns="http://www.w3.org/2000/svg"><style>#class-context-positive{font-size:16px}.classLabel .label{font-size:10px}</style><g class="classLabel"><g class="label"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains("font-size: 10px"),
            "a fully matching contextual selector must remain effective: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_does_not_match_svg_text_selector_to_xhtml_text() {
        let svg = r#"<svg id="element-context" xmlns="http://www.w3.org/2000/svg"><style>#element-context{font-size:16px}g.classGroup text{font-size:10px}</style><g class="classGroup"><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains("font-size: 16px"),
            "an SVG text selector must not match an XHTML span target: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_inherits_font_size_presentation_attribute() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g font-size="20px"><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span>Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains("font-size: 20px"),
            "font-size presentation attributes must participate in inheritance: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_uses_nested_xhtml_font_style() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml" style="line-height:1.5"><span style="font-size:18px;font-style:italic">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(out.contains("font-size: 18px"), "got: {out}");
        assert!(out.contains("font-style: italic"), "got: {out}");
    }

    #[test]
    fn foreign_object_overlay_matches_admitted_attribute_selectors() {
        let svg = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>[data-fallback-role=label][data-tags~=choice]{font-size:13px}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span data-fallback-role='label' data-tags="primary choice">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(out.contains("font-size: 13px"), "got: {out}");
    }

    #[test]
    fn foreign_object_overlay_keeps_admitted_selector_siblings_and_rejects_invalid_lists() {
        let valid_but_unadmitted = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>span:hover,.nodeLabel{font-size:13px}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(valid_but_unadmitted);
        assert!(
            out.contains("font-size: 13px"),
            "a valid-but-unadmitted sibling must not discard an admitted branch: {out}"
        );

        let invalid = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>.nodeLabel,{font-size:13px}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(invalid);
        assert!(
            out.contains("font-size: 16px"),
            "an invalid ordinary selector list must discard its complete rule: {out}"
        );

        let malformed_combinator = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>span >> .nodeLabel{font-size:10px}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(malformed_combinator);
        assert!(
            out.contains("font-size: 16px") && !out.contains("font-size: 10px"),
            "malformed repeated child combinators must fail closed: {out}"
        );

        let malformed_compound = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>.nodeLabel,$bad{font-size:10px}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(malformed_compound);
        assert!(
            out.contains("font-size: 16px") && !out.contains("font-size: 10px"),
            "an invalid compound must invalidate its complete selector list: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_skips_nested_at_rules_without_leaking_inner_selectors() {
        let svg = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>@media screen {.nodeLabel{font-size:10px}} .nodeLabel{font-size:14px}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);
        assert!(
            out.contains("font-size: 14px"),
            "nested @media rules are outside the admitted fallback cascade and must not leak: {out}"
        );
        assert!(
            !out.contains("font-size: 10px"),
            "an inner at-rule selector must not be parsed as a top-level rule: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_resolves_important_inline_and_invalid_value_cascade() {
        let stylesheet_important = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>.nodeLabel{font-size:12px !important}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel" style="font-size:14px">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(stylesheet_important);
        assert!(out.contains("font-size: 12px"), "got: {out}");

        let inline_important = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>.nodeLabel{font-size:12px !important}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel" style="font-size:14px !important">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(inline_important);
        assert!(out.contains("font-size: 14px"), "got: {out}");

        let unsupported_winner = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>.nodeLabel{font-size:14px;font-size:calc(2px) !important}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(unsupported_winner);
        assert!(
            out.contains("font-size: 14px"),
            "an unsupported high-priority value must not erase a lower valid declaration: {out}"
        );

        let invalid_non_metric_winner = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>.nodeLabel{font-weight:600;font-weight:9999 !important;font-style:italic;font-style:bogus !important;fill:#123456;fill:definitely-not-a-paint !important;color:#abcdef;color:none !important;color:VAR(--missing) !important}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(invalid_non_metric_winner);
        let fallback = out
            .split(r#"data-merman-foreignobject="fallback""#)
            .nth(1)
            .unwrap_or_else(|| panic!("expected fallback output: {out}"));
        assert!(fallback.contains("font-weight: 600"), "got: {out}");
        assert!(fallback.contains("font-style: italic"), "got: {out}");
        assert!(
            fallback.contains(r##"fill="#abcdef""##),
            "XHTML fallback text should prefer CSS color over SVG fill: {out}"
        );
        assert!(
            !fallback.contains("9999"),
            "invalid font weight leaked: {out}"
        );
        assert!(
            !fallback.contains("bogus"),
            "invalid font style leaked: {out}"
        );
        assert!(
            !fallback.contains("definitely-not-a-paint"),
            "invalid fill leaked: {out}"
        );
        assert!(
            !fallback.contains("VAR("),
            "case-insensitive var() leaked: {out}"
        );

        let invalid_relative_values = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>.nodeLabel{font-size:14px;line-height:1.5}.nodeLabel{font-size:20 px !important;line-height:1e308 !important}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(invalid_relative_values);
        let fallback = out
            .split(r#"data-merman-foreignobject="fallback""#)
            .nth(1)
            .unwrap_or_else(|| panic!("expected fallback output: {out}"));
        assert!(fallback.contains("font-size: 14px"), "got: {out}");
        assert!(fallback.contains("line-height: 21px"), "got: {out}");
        assert!(
            !fallback.contains("20 px") && !fallback.contains("inf"),
            "non-finite metric leaked: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_keeps_late_universal_winners() {
        let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg"><style>"#);
        for _ in 0..4096 {
            svg.push_str("*{font-size:12px}");
        }
        svg.push_str(
            r#"*{font-size:19px}</style><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span>Alpha</span></div></foreignObject></svg>"#,
        );

        let out = foreign_object_label_fallback_svg_text(&svg);
        assert!(
            out.contains("font-size: 19px"),
            "a late admitted universal rule must not disappear from the cascade: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_resolves_specified_values_before_inheritance_and_source_order() {
        let inherited_important = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>#root{font-size:20px !important}.nodeLabel{font-size:14px}</style><g id="root"><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(inherited_important);
        assert!(
            out.contains("font-size: 14px"),
            "a child's specified normal value must win before inheriting a parent's important value: {out}"
        );

        let presentation_vs_stylesheet = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>.host{font-size:12px}</style><g class="host" font-size="20px"><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span>Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(presentation_vs_stylesheet);
        assert!(
            out.contains("font-size: 12px"),
            "stylesheet declarations must follow presentation attributes in author source order: {out}"
        );

        let source_order = r#"
<svg xmlns="http://www.w3.org/2000/svg"><style>.nodeLabel{font-size:12px}.nodeLabel{font-size:14px}</style><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml"><span class="nodeLabel">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(source_order);
        assert!(
            out.contains("font-size: 14px"),
            "later equal-specificity declarations must win by stylesheet source order: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_uses_root_rem_and_measurement_matches_emission() {
        let svg = r#"
<svg style="font-size:20px" xmlns="http://www.w3.org/2000/svg"><g style="font-size:10px"><foreignObject width="80" height="60"><div xmlns="http://www.w3.org/1999/xhtml" style="white-space:normal;width:80px;line-height:1.5"><span style="font-size:2rem;font-family:Inter;font-weight:600;font-style:italic">Alpha</span></div></foreignObject></g></svg>"#;
        let measurer = RecordingMeasurer::default();
        let out = render_fallback(svg, &measurer);
        let styles = measurer.styles.borrow();

        assert!(
            styles.iter().any(|style| {
                style.font_size == 40.0
                    && style.font_family.as_deref() == Some("Inter")
                    && style.font_weight.as_deref() == Some("600")
                    && style.font_style.as_deref() == Some("italic")
            }),
            "measurement must receive the resolved typography: {styles:?}"
        );
        assert!(out.contains("font-size: 40px"), "got: {out}");
        assert!(out.contains("font-family: Inter"), "got: {out}");
        assert!(out.contains("font-weight: 600"), "got: {out}");
        assert!(out.contains("font-style: italic"), "got: {out}");
        assert!(out.contains("line-height: 60px"), "got: {out}");
    }

    #[test]
    fn foreign_object_overlay_preserves_inherited_line_height_forms() {
        let multiplier = r#"
<svg xmlns="http://www.w3.org/2000/svg"><g><foreignObject width="80" height="48"><div xmlns="http://www.w3.org/1999/xhtml" style="line-height:1.5"><span style="font-size:20px">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(multiplier);
        assert!(
            out.contains("font-size: 20px") && out.contains("line-height: 30px"),
            "a unitless inherited line-height must scale at the child font size: {out}"
        );

        let absolute = r#"
<svg xmlns="http://www.w3.org/2000/svg"><g><foreignObject width="80" height="48"><div xmlns="http://www.w3.org/1999/xhtml" style="line-height:24px"><span style="font-size:20px">Alpha</span></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(absolute);
        assert!(
            out.contains("font-size: 20px") && out.contains("line-height: 24px"),
            "an absolute inherited line-height must remain absolute: {out}"
        );
    }

    #[test]
    fn foreign_object_overlay_uses_common_ancestor_for_mixed_text_leaves() {
        let svg = r#"
<svg xmlns="http://www.w3.org/2000/svg"><g><foreignObject width="80" height="30"><div xmlns="http://www.w3.org/1999/xhtml" style="font-size:14px"><span>Alpha</span><strong style="font-size:20px">Beta</strong></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);

        assert!(
            out.contains("font-size: 14px"),
            "mixed text leaves should use their deepest common ancestor's fallback typography: {out}"
        );
        assert!(
            !out.contains("font-size: 20px"),
            "the fallback must not arbitrarily select one rich-text leaf: {out}"
        );
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

        assert!(text_tag.contains(r##"fill="#665c54""##), "got: {out}");
        assert!(
            !text_tag.contains(r##"fill="#ebdbb2""##),
            "the SVG-only `text` branch must not match the XHTML source leaf: {out}"
        );
        assert!(
            text_tag.contains(r#"class="merman-foreignobject-fallback-text""#)
                && text_tag.contains(r#"data-merman-source-classes="edgeLabel label labelBkg""#),
            "fallback text should retain source classes without making them live selectors: {out}"
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
