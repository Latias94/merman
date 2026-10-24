//! A bounded projection of RaTeX-owned nowrap rows, not an HTML or arbitrary SVG layout engine.

use super::cascade::ResolvedFallbackTypography;
use crate::svg::pipeline::{
    escape_xml_attr_with_checkpoints, escape_xml_text_with_checkpoints, find_with_checkpoints,
};
use crate::text::{TextMeasurer, is_html_collapsible_ascii_whitespace};
use quick_xml::{Reader, XmlVersion, events::Event};

const ROW_CLASS: &str = "merman-native-math-row";
const SVG_CLASS: &str = "merman-native-math-svg";

pub(super) struct NativeMathProjection {
    pub(super) svg: String,
    pub(super) elements: usize,
    pub(super) depth: usize,
}

enum Run<'a> {
    Text {
        text: String,
        width: f64,
        height: f64,
    },
    Svg {
        body: &'a str,
        view_box: String,
        width: f64,
        height: f64,
        elements: usize,
    },
}

impl Run<'_> {
    fn size(&self) -> (f64, f64) {
        match self {
            Self::Text { width, height, .. } | Self::Svg { width, height, .. } => (*width, *height),
        }
    }
}

#[derive(Default)]
struct Row<'a> {
    runs: Vec<Run<'a>>,
    width: f64,
    height: f64,
}

struct OpenSvg {
    body_start: usize,
    source_depth: usize,
    view_box: String,
    width: f64,
    height: f64,
    elements: usize,
}

fn reject<T, E>(
    message: &str,
    invalid: &mut impl FnMut(&str) -> Result<(), E>,
) -> Result<Option<T>, E> {
    invalid(message)?;
    Ok(None)
}

fn dimension(value: &str) -> Option<f64> {
    let value = value.parse::<f64>().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn view_box_dimensions(value: &str) -> Option<(f64, f64)> {
    let mut parts = value.split_ascii_whitespace();
    let x = dimension(parts.next()?)?;
    let y = dimension(parts.next()?)?;
    let width = dimension(parts.next()?)?;
    let height = dimension(parts.next()?)?;
    (x == 0.0 && y == 0.0 && parts.next().is_none()).then_some((width, height))
}

// This list follows ratex-svg's outline, glyph-text, rule, and dashed-rule emitters.
fn primitive_attribute(element: &str, key: &str, value: &str) -> bool {
    match (element, key) {
        ("path", "d") => true,
        ("path", "fill-rule") => value == "nonzero",
        ("path", "stroke-linecap" | "stroke-linejoin") => value == "round",
        ("path" | "rect" | "text", "fill") | ("path" | "line", "stroke") => {
            value == "none" || value.starts_with("rgba(") || value.starts_with("rgb(")
        }
        ("path" | "rect" | "text", "fill-opacity")
        | ("path" | "line", "stroke-opacity" | "stroke-width")
        | ("rect", "width" | "height")
        | ("text", "font-size") => dimension(value).is_some(),
        ("rect" | "text", "x" | "y") | ("line", "x1" | "y1" | "x2" | "y2") => {
            value.parse::<f64>().is_ok_and(f64::is_finite)
        }
        ("text", "font-family") => !value.is_empty(),
        ("text", "font-weight") => matches!(value, "normal" | "bold"),
        ("text", "font-style") => matches!(value, "normal" | "italic"),
        ("text", "dominant-baseline") => value == "alphabetic",
        ("line", "stroke-dasharray") => {
            let mut parts = value.split_ascii_whitespace();
            parts.next().and_then(dimension).is_some()
                && parts.next().and_then(dimension).is_some()
                && parts.next().is_none()
        }
        _ => false,
    }
}

fn has_owned_markup<E>(
    inner: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    let mut reader = Reader::from_str(inner);
    loop {
        checkpoint()?;
        match reader.read_event() {
            Ok(Event::Start(tag) | Event::Empty(tag)) => {
                for attribute in tag.attributes().with_checks(true) {
                    checkpoint()?;
                    let Ok(attribute) = attribute else { continue };
                    if attribute.key.as_ref() == "class"
                        && let Ok(value) = attribute.normalized_value(XmlVersion::Implicit1_0)
                        && value
                            .split_ascii_whitespace()
                            .any(|token| matches!(token, ROW_CLASS | SVG_CLASS))
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return Ok(false),
            _ => {}
        }
    }
}

fn flush_text<E>(
    text: &mut String,
    row: &mut Row<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    // Each anonymous flex text item collapses ASCII whitespace and trims its own edges.
    // NBSP is deliberately not HTML-collapsible whitespace.
    let mut collapsed = String::new();
    let mut space = false;
    for ch in text.chars() {
        checkpoint()?;
        if is_html_collapsible_ascii_whitespace(ch) {
            space = !collapsed.is_empty();
        } else {
            if space {
                collapsed.push(' ');
            }
            collapsed.push(ch);
            space = false;
        }
    }
    text.clear();
    if !collapsed.is_empty() {
        row.runs.push(Run::Text {
            text: collapsed,
            width: 0.0,
            height: 0.0,
        });
    }
    Ok(())
}

fn parse_rows<'a, E>(
    inner: &'a str,
    font_size: f64,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
    invalid: &mut impl FnMut(&str) -> Result<(), E>,
) -> Result<Option<Vec<Row<'a>>>, E> {
    let mut reader = Reader::from_str(inner);
    reader.config_mut().check_end_names = true;
    let mut rows = Vec::new();
    let mut row = None::<Row<'a>>;
    let mut row_depth = 0;
    let mut svg = None::<OpenSvg>;
    let mut glyph_text_depth = None;
    let mut depth = 0usize;
    let mut text = String::new();

    loop {
        checkpoint()?;
        let event_start = reader.buffer_position() as usize;
        let event = reader.read_event();
        checkpoint()?;
        let event = match event {
            Ok(event) => event,
            Err(_) => return reject("native math requires balanced XML", invalid),
        };
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(tag) | Event::Empty(tag) => {
                let next_depth = depth.saturating_add(1);
                if next_depth > crate::resources::MAX_RESVG_TREE_DEPTH {
                    return reject(
                        "native math source nesting exceeds the supported depth",
                        invalid,
                    );
                }
                let name = tag.name();
                let name = name.as_ref();
                let in_svg = svg.is_some();
                if in_svg {
                    if !matches!(name, "path" | "rect" | "line" | "text")
                        || svg
                            .as_ref()
                            .is_some_and(|open| next_depth != open.source_depth + 1)
                    {
                        return reject("unsupported native math SVG structure", invalid);
                    }
                } else if !matches!(name, "div" | "span" | "svg") {
                    return reject("unsupported native math XHTML structure", invalid);
                }

                let mut owned_row = false;
                let mut owned_svg = false;
                let mut view_box = None;
                let mut width_em = None;
                let mut height_em = None;
                for attribute in tag.attributes().with_checks(true) {
                    checkpoint()?;
                    let Ok(attribute) = attribute else {
                        return reject("invalid native math XML attribute", invalid);
                    };
                    let Ok(value) = attribute.normalized_value(XmlVersion::Implicit1_0) else {
                        return reject("invalid native math XML attribute value", invalid);
                    };
                    let key = attribute.key.as_ref();
                    if in_svg {
                        if !primitive_attribute(name, key, &value) {
                            return reject("unsupported native math SVG attribute", invalid);
                        }
                        continue;
                    }
                    match key {
                        "class" => {
                            for token in value.split_ascii_whitespace() {
                                checkpoint()?;
                                owned_row |= token == ROW_CLASS;
                                owned_svg |= token == SVG_CLASS;
                            }
                        }
                        "viewBox" if name == "svg" => view_box = Some(value.into_owned()),
                        "width" if name == "svg" => {
                            width_em = value.strip_suffix("em").and_then(dimension);
                        }
                        "height" if name == "svg" => {
                            height_em = value.strip_suffix("em").and_then(dimension);
                        }
                        "xmlns" => {
                            let expected = if name == "svg" {
                                "http://www.w3.org/2000/svg"
                            } else {
                                "http://www.w3.org/1999/xhtml"
                            };
                            if value != expected {
                                return reject("unsupported native math namespace", invalid);
                            }
                        }
                        _ if name == "svg" => {
                            return reject("unsupported native math SVG root attribute", invalid);
                        }
                        _ => {}
                    }
                }

                if let Some(open) = svg.as_mut() {
                    open.elements = open.elements.saturating_add(1);
                    if name == "text" && !empty {
                        glyph_text_depth = Some(next_depth);
                    }
                } else if owned_row {
                    if name != "div" || owned_svg || row.is_some() {
                        return reject("native math rows must be non-nested owned divs", invalid);
                    }
                    row = Some(Row::default());
                    row_depth = next_depth;
                    if empty {
                        if let Some(row) = row.take() {
                            rows.push(row);
                        }
                    }
                } else if name == "svg" {
                    let Some(current_row) = row.as_mut() else {
                        return reject("native math SVG must be inside an owned row", invalid);
                    };
                    if !owned_svg || next_depth != row_depth + 1 {
                        return reject(
                            "native math SVG must be an owned direct row child",
                            invalid,
                        );
                    }
                    let Some(view_box) = view_box else {
                        return reject("native math SVG requires an intrinsic viewBox", invalid);
                    };
                    let Some((vw, vh)) = view_box_dimensions(&view_box) else {
                        return reject(
                            "native math viewBox must have finite nonnegative dimensions and zero origin",
                            invalid,
                        );
                    };
                    if width_em != Some(vw) || height_em != Some(vh) {
                        return reject(
                            "native math SVG em dimensions must match its viewBox",
                            invalid,
                        );
                    }
                    let (width, height) = (vw * font_size, vh * font_size);
                    if !width.is_finite() || !height.is_finite() {
                        return reject(
                            "native math intrinsic pixel dimensions overflowed",
                            invalid,
                        );
                    }
                    flush_text(&mut text, current_row, checkpoint)?;
                    let open = OpenSvg {
                        body_start: reader.buffer_position() as usize,
                        source_depth: next_depth,
                        view_box,
                        width,
                        height,
                        elements: 1,
                    };
                    if empty {
                        current_row.runs.push(Run::Svg {
                            body: "",
                            view_box: open.view_box,
                            width,
                            height,
                            elements: 1,
                        });
                    } else {
                        svg = Some(open);
                    }
                } else if owned_svg || row.is_some() {
                    return reject("unsupported native math row child", invalid);
                }
                if !empty {
                    depth = next_depth;
                }
            }
            Event::End(_) => {
                if depth == 0 {
                    return reject("native math requires balanced XML", invalid);
                }
                if svg.as_ref().is_some_and(|open| depth == open.source_depth) {
                    if let (Some(open), Some(row)) = (svg.take(), row.as_mut()) {
                        row.runs.push(Run::Svg {
                            body: &inner[open.body_start..event_start],
                            view_box: open.view_box,
                            width: open.width,
                            height: open.height,
                            elements: open.elements,
                        });
                    }
                } else if row.is_some() && depth == row_depth {
                    if let Some(mut row) = row.take() {
                        flush_text(&mut text, &mut row, checkpoint)?;
                        rows.push(row);
                    }
                }
                if glyph_text_depth == Some(depth) {
                    glyph_text_depth = None;
                }
                depth -= 1;
            }
            Event::Text(value) => {
                let value = value.xml10_content();
                if svg.is_none() && row.is_some() {
                    text.push_str(&value);
                } else if glyph_text_depth != Some(depth) {
                    for ch in value.chars() {
                        checkpoint()?;
                        if !is_html_collapsible_ascii_whitespace(ch) {
                            return reject("native math text must be in an owned row", invalid);
                        }
                    }
                }
            }
            Event::GeneralRef(reference) if svg.is_none() && row.is_some() => {
                let reference = format!("&{};", reference.as_ref());
                let Ok(value) = quick_xml::escape::unescape(&reference) else {
                    return reject("unsupported native math text entity", invalid);
                };
                text.push_str(&value);
            }
            Event::GeneralRef(reference) if glyph_text_depth == Some(depth) => {
                let reference = format!("&{};", reference.as_ref());
                if quick_xml::escape::unescape(&reference).is_err() {
                    return reject("unsupported native math glyph entity", invalid);
                }
            }
            Event::Eof => {
                if depth != 0 || row.is_some() || svg.is_some() {
                    return reject("native math requires balanced XML", invalid);
                }
                break;
            }
            _ => return reject("unsupported native math XML event", invalid),
        }
    }
    Ok(Some(rows))
}

pub(super) fn project_native_math<E>(
    inner: &str,
    typography: &ResolvedFallbackTypography,
    width: f64,
    height: f64,
    text_measurer: &dyn TextMeasurer,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
    preflight: &mut impl FnMut(usize, usize, usize) -> Result<(), E>,
    invalid: &mut impl FnMut(&str) -> Result<(), E>,
) -> Result<Option<NativeMathProjection>, E> {
    checkpoint()?;
    if find_with_checkpoints(inner, ROW_CLASS, checkpoint)?.is_none()
        && find_with_checkpoints(inner, SVG_CLASS, checkpoint)?.is_none()
    {
        return Ok(None);
    }
    if !has_owned_markup(inner, checkpoint)? {
        return Ok(None);
    }
    if [width, height, typography.font_size, typography.line_height]
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return reject(
            "native math dimensions and typography must be finite and nonnegative",
            invalid,
        );
    }
    let Some(mut rows) = parse_rows(inner, typography.font_size, checkpoint, invalid)? else {
        return Ok(None);
    };
    if rows.is_empty() {
        return Ok(None);
    }
    let style = typography.text_style();
    let mut total_height = 0.0;
    for row in &mut rows {
        checkpoint()?;
        let mut has_text = false;
        for run in &mut row.runs {
            checkpoint()?;
            if let Run::Text {
                text,
                width,
                height,
            } = run
            {
                *width = text_measurer.measure_svg_raw_text_bbox_width_px(text, &style);
                checkpoint()?;
                *height = text_measurer.measure_svg_raw_text_bbox_height_px(text, &style);
                checkpoint()?;
                has_text = true;
            }
            let (run_width, run_height) = run.size();
            if !run_width.is_finite()
                || !run_height.is_finite()
                || run_width < 0.0
                || run_height < 0.0
            {
                return reject(
                    "native math text measurements must be finite and nonnegative",
                    invalid,
                );
            }
            row.width += run_width;
            row.height = row.height.max(run_height);
        }
        if has_text {
            row.height = row.height.max(typography.line_height);
        }
        total_height += row.height;
        if !row.width.is_finite() || !total_height.is_finite() {
            return reject("native math row dimensions overflowed", invalid);
        }
    }

    let fill = escape_xml_attr_with_checkpoints(&typography.fill, checkpoint)?;
    // Like ordinary fallback text, resolved typography must override diagram text selectors.
    let text_style = format!(
        "font-size: {}px; font-family: {}; fill: {}; font-weight: {}; font-style: {}; text-anchor: start; dominant-baseline: central; alignment-baseline: central; white-space: pre; stroke: none;",
        typography.font_size,
        typography.font_family,
        typography.fill,
        typography.font_weight.as_deref().unwrap_or("normal"),
        typography.font_style.as_deref().unwrap_or("normal"),
    );
    let text_style = escape_xml_attr_with_checkpoints(&text_style, checkpoint)?;
    let mut output = String::new();
    let mut elements = 0usize;
    let mut generated_depth = 1usize;
    super::push_generated_fmt(
        &mut output,
        format_args!(r#"<g class="merman-prepared-math-native">"#),
        1,
        0,
        generated_depth,
        &mut elements,
        preflight,
    )?;
    let mut y = (height - total_height) / 2.0;
    for row in rows {
        checkpoint()?;
        let mut x = (width - row.width) / 2.0;
        for run in row.runs {
            checkpoint()?;
            let (run_width, run_height) = run.size();
            let top = y + (row.height - run_height) / 2.0;
            if !x.is_finite() || !top.is_finite() {
                return reject("native math run positions overflowed", invalid);
            }
            match run {
                Run::Text { text, .. } => {
                    let center = top + run_height / 2.0;
                    if !center.is_finite() {
                        return reject("native math text position overflowed", invalid);
                    }
                    let escaped = escape_xml_text_with_checkpoints(&text, checkpoint)?;
                    generated_depth = generated_depth.max(2);
                    super::push_generated_fmt(
                        &mut output,
                        format_args!(
                            r#"<text x="{x}" y="{center}" fill="{fill}" xml:space="preserve" style="{text_style}">{escaped}</text>"#
                        ),
                        1,
                        0,
                        generated_depth,
                        &mut elements,
                        preflight,
                    )?;
                }
                Run::Svg {
                    body,
                    view_box,
                    elements: svg_elements,
                    ..
                } => {
                    let view_box = escape_xml_attr_with_checkpoints(&view_box, checkpoint)?;
                    generated_depth = generated_depth.max(if svg_elements > 1 { 3 } else { 2 });
                    super::push_generated_fmt(
                        &mut output,
                        format_args!(
                            r#"<svg class="merman-native-math-svg" x="{x}" y="{top}" width="{run_width}" height="{run_height}" viewBox="{view_box}" overflow="visible">{body}</svg>"#
                        ),
                        svg_elements,
                        0,
                        generated_depth,
                        &mut elements,
                        preflight,
                    )?;
                }
            }
            x += run_width;
        }
        y += row.height;
    }
    checkpoint()?;
    super::push_generated_fmt(
        &mut output,
        format_args!("</g>"),
        0,
        0,
        generated_depth,
        &mut elements,
        preflight,
    )?;
    Ok(Some(NativeMathProjection {
        svg: output,
        elements,
        depth: generated_depth,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{TextMetrics, TextStyle};

    struct Measurer;

    impl TextMeasurer for Measurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            panic!("native math must use the raw SVG bbox operations")
        }

        fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            assert_eq!(style.font_size, 10.0);
            text.chars().count() as f64 * 5.0
        }

        fn measure_svg_raw_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            8.0
        }
    }

    fn typography() -> ResolvedFallbackTypography {
        ResolvedFallbackTypography {
            font_size: 10.0,
            font_family: "A&B".into(),
            font_weight: Some("700".into()),
            font_style: Some("italic".into()),
            line_height: 15.0,
            fill: "#123456".into(),
            label_background: None,
        }
    }

    fn formula() -> &'static str {
        r#"<svg class='merman-native-math-svg' viewBox='0 0 2 1' width='2em' height='1em'><path d='M0 0L1 1' fill='rgba(0,0,0,1)'/></svg>"#
    }

    fn project(inner: &str) -> Result<Option<NativeMathProjection>, &'static str> {
        project_native_math(
            inner,
            &typography(),
            100.0,
            60.0,
            &Measurer,
            &mut || Ok(()),
            &mut |_, _, _| Ok(()),
            &mut |_| Err("invalid"),
        )
    }

    #[test]
    fn unowned_content_declines_before_measurement_or_geometry_validation() {
        assert!(project("<div>ordinary label</div>").unwrap().is_none());
        for inner in [
            "<div>merman-native-math-row</div>",
            "<div><span>merman-native-math-svg</span></div>",
            "<div class='other-merman-native-math-row'>ordinary</div>",
        ] {
            assert!(project(inner).unwrap().is_none(), "{inner}");
        }
        let result = project_native_math(
            "<svg/>",
            &typography(),
            f64::NAN,
            -1.0,
            &Measurer,
            &mut || Ok::<_, ()>(()),
            &mut |_, _, _| Ok(()),
            &mut |_| panic!("unowned input"),
        );
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn pure_svg_uses_intrinsic_height_and_reports_exact_structure() {
        let inner = format!(
            "<div><span><div class='{ROW_CLASS}'>{}</div></span></div>",
            formula()
        );
        let result = project(&inner).unwrap().unwrap();
        assert!(result.svg.contains(
            r#"x="40" y="25" width="20" height="10" viewBox="0 0 2 1" overflow="visible""#
        ));
        assert!(result.svg.contains("d='M0 0L1 1'"));
        assert_eq!(result.elements, 3);
        assert_eq!(result.depth, 3);
    }

    #[test]
    fn empty_owned_rows_do_not_move_the_formula() {
        let base = format!("<div class='{ROW_CLASS}'>{}</div>", formula());
        let padded = format!(
            "<div class='{ROW_CLASS}'/><div class='{ROW_CLASS}'> </div>{base}<div class='{ROW_CLASS}'/>"
        );
        assert_eq!(
            project(&padded).unwrap().unwrap().svg,
            project(&base).unwrap().unwrap().svg
        );
    }

    #[test]
    fn owned_svg_keeps_emitter_glyph_text_and_entities() {
        let inner = format!(
            "<div class='{ROW_CLASS}'><svg class='{SVG_CLASS}' viewBox='0 0 2 1' width='2em' height='1em'><text x='0' y='1' font-family='sans-serif' font-size='1' font-weight='normal' font-style='normal' fill='rgba(0,0,0,1)' dominant-baseline='alphabetic'>&lt;</text><path d='M0 0L1 1' fill='rgba(0,0,0,1)'/></svg></div>"
        );
        let result = project(&inner).unwrap().unwrap();
        assert!(
            result
                .svg
                .contains("dominant-baseline='alphabetic'>&lt;</text>")
        );
        assert_eq!(result.elements, 4);
        assert_eq!(result.depth, 3);
    }

    #[test]
    fn mixed_runs_and_owned_plain_rows_keep_order_and_line_height() {
        let inner = format!(
            "<div class='{ROW_CLASS}'>  before\t &amp; {} between {} after </div><div class='{ROW_CLASS}'>plain</div>",
            formula(),
            formula()
        );
        let result = project(&inner).unwrap().unwrap();
        assert!(result.svg.contains(">before &amp;</text>"));
        assert!(result.svg.contains(">between</text>"));
        assert!(result.svg.contains(">after</text>"));
        assert!(result.svg.contains(">plain</text>"));
        assert!(result.svg.contains(r#"x="37.5" y="37.5""#));
        assert!(result.svg.contains("font-family: A&amp;B;"));
        assert!(result.svg.contains("font-weight: 700; font-style: italic;"));
        assert_eq!(result.elements, 9);
        assert_eq!(result.depth, 3);
    }

    #[test]
    fn entities_and_nonbreaking_spaces_survive_without_double_escaping() {
        let inner = format!("<div class='{ROW_CLASS}'>a&#160;b &lt;x&gt; &quot;q&quot;</div>");
        let result = project(&inner).unwrap().unwrap();
        assert!(result.svg.contains("a\u{a0}b &lt;x&gt; \"q\"</text>"));
        assert_eq!(result.elements, 2);
        assert_eq!(result.depth, 2);
    }

    #[test]
    fn rejects_unbalanced_unowned_or_unsupported_owned_structures() {
        for inner in [
            format!("<div class='{ROW_CLASS}'><span>unsupported</span></div>"),
            format!("<div class='{ROW_CLASS}'>{}</span>", formula()),
            format!("<div class='{ROW_CLASS}'>{}", formula()),
            format!("outside<div class='{ROW_CLASS}'>{}</div>", formula()),
            format!(
                "<div class='{ROW_CLASS}'><svg viewBox='0 0 2 1' width='2em' height='1em'/></div>"
            ),
            format!("<div class='{ROW_CLASS}'><div class='{ROW_CLASS}'/></div>"),
            format!(
                "<div class='{ROW_CLASS}'>{}</div>",
                formula().replace("<path", "<script")
            ),
            format!(
                "<div class='{ROW_CLASS}'>{}</div>",
                formula().replace(" d=", " onclick='bad' d=")
            ),
        ] {
            assert!(matches!(project(&inner), Err("invalid")), "{inner}");
        }
    }

    #[test]
    fn rejects_nonfinite_negative_and_mismatched_intrinsic_dimensions() {
        for formula in [
            formula().replace("0 0 2 1", "0 0 NaN 1"),
            formula().replace("0 0 2 1", "0 0 -2 1"),
            formula().replace("width='2em'", "width='3em'"),
            formula().replace("height='1em'", "height='infem'"),
        ] {
            assert!(matches!(
                project(&format!("<div class='{ROW_CLASS}'>{formula}</div>")),
                Err("invalid")
            ));
        }
    }

    #[test]
    fn checkpoint_and_invalid_callback_errors_propagate() {
        let inner = format!("<div class='{ROW_CLASS}'>{}</div>", formula());
        let result = project_native_math(
            &inner,
            &typography(),
            100.0,
            60.0,
            &Measurer,
            &mut || Err::<(), _>("cancelled"),
            &mut |_, _, _| Ok(()),
            &mut |_| Err("invalid"),
        );
        assert!(matches!(result, Err("cancelled")));
        let result = project_native_math(
            &inner,
            &typography(),
            -1.0,
            60.0,
            &Measurer,
            &mut || Ok::<_, ()>(()),
            &mut |_, _, _| Ok(()),
            &mut |_| Ok(()),
        );
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn generated_budget_is_checked_before_each_amplified_run_is_written() {
        let inner = format!(
            "<div class='{ROW_CLASS}'>{}</div>",
            format!("text {}", formula()).repeat(128)
        );
        let mut typography = typography();
        typography.font_family = "A".repeat(4096);
        let mut checked = Vec::new();
        let result = project_native_math(
            &inner,
            &typography,
            100.0,
            60.0,
            &Measurer,
            &mut || Ok(()),
            &mut |bytes, elements, depth| {
                checked.push((bytes, elements, depth));
                if bytes > 1024 { Err("budget") } else { Ok(()) }
            },
            &mut |_| Err("invalid"),
        );
        assert!(matches!(result, Err("budget")));
        assert_eq!(checked.len(), 2, "stop before writing the first text run");
        assert_eq!(checked[0].1, 1);
        assert_eq!(checked[1].1, 2);
        assert!(checked[1].0 > 4096);
    }
}
