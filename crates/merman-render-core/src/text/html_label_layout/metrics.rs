use super::{flowchart_html_has_inline_style_tags, flowchart_html_label_box};
use crate::mermaid_style::HtmlTextGeometryDeclarations;
use crate::text::{
    TextMeasurer, TextStyle, WrapMode,
    create_text_html::{FlowchartLabelMetricsRequest, is_single_img_label},
    html_label_box::HtmlLabelWidth,
    inline_html::{measure_html_with_authored_width, measure_html_with_inline_styles},
    is_html_collapsible_ascii_whitespace,
    literal_label::{flowchart_decode_label_escapes, flowchart_non_markdown_svg_plain_text},
    styled_html::measure_styled_html_label,
    trim_html_collapsible_ascii_whitespace,
};
use quick_xml::{
    Reader, XmlVersion,
    events::{BytesStart, Event},
};
use std::borrow::Cow;

/// Measures an already-sanitized Flow-style HTML label using its original layout context.
pub fn measure_flowchart_html_label(
    request: FlowchartLabelMetricsRequest<'_>,
    xhtml: &str,
    geometry: HtmlTextGeometryDeclarations,
    probe_intrinsic_images: bool,
) -> crate::text::TextMetrics {
    let dimensions = flowchart_html_label_box(request.measurer, xhtml);
    let width = dimensions.and_then(|dimensions| dimensions.width_in_label(request.max_width_px));
    let mut metrics = if let Some(metrics) = measure_styled_html_label(
        request.measurer,
        xhtml,
        request.style,
        geometry,
        width.map_or(request.max_width_px, |width| width.wrap),
        width.is_some(),
    ) {
        metrics
    } else if let Some(width) = width
        && !xhtml.contains("<img")
    {
        measure_html_with_authored_width(request.measurer, xhtml, request.style, width.wrap)
    } else {
        flowchart_html_label_content_metrics(
            request,
            xhtml,
            geometry,
            width,
            probe_intrinsic_images,
        )
    };
    if let Some(width) = width {
        metrics.width = width.outer;
    }
    if let Some(dimensions) = dimensions {
        metrics.height = dimensions.height.unwrap_or(metrics.height);
    }
    metrics
}

fn flowchart_html_label_content_metrics(
    request: FlowchartLabelMetricsRequest<'_>,
    xhtml: &str,
    geometry: HtmlTextGeometryDeclarations,
    authored_width: Option<HtmlLabelWidth>,
    probe_intrinsic_images: bool,
) -> crate::text::TextMetrics {
    let FlowchartLabelMetricsRequest {
        measurer,
        raw_label,
        label_type,
        style,
        max_width_px,
        ..
    } = request;
    if xhtml.is_empty() {
        return crate::text::TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        };
    }
    let has_inline_style = xhtml.contains(" style=\"")
        && xhtml.split('<').skip(1).any(|element| {
            element
                .split_once('>')
                .is_some_and(|(tag, _)| tag.contains(" style=\""))
        });
    if !has_inline_style
        && let Some(plain) = crate::text::mermaid_xhtml_label_plain_text(xhtml)
        && !plain.contains('<')
    {
        // A host profile may provide the complete paragraph's browser measurement. Keep that
        // request intact after collapsing rendered whitespace, instead of probing each word.
        let plain = match plain {
            Cow::Borrowed(text) => flowchart_literal_paragraph_for_layout(text),
            Cow::Owned(_) => Cow::Owned(flowchart_label_plain_text_for_layout(xhtml, "html", true)),
        };
        let mut metrics = measurer.measure_wrapped(&plain, style, max_width_px, WrapMode::HtmlLike);
        if label_type == "markdown" {
            let max_width = max_width_px.filter(|width| width.is_finite() && *width > 0.0);
            if let Some(max_width) = max_width
                && measurer
                    .measure_wrapped(&plain, style, None, WrapMode::HtmlLike)
                    .width
                    > max_width
            {
                metrics.width = metrics.width.max(max_width);
            } else {
                metrics.width = crate::text::round_to_1_64_px(metrics.width);
                if let Some(max_width) = max_width {
                    metrics.width = metrics.width.min(max_width);
                }
            }
        }
        return metrics;
    }
    let lower = xhtml.to_ascii_lowercase();
    // Marked leaves unsupported blocks as root text alongside any generated paragraphs. Keep
    // that plain-text measurement path; ordinary Markdown paragraphs use the browser's width
    // lattice. Read the final sanitized structure so removed markup cannot affect this choice.
    let mut markdown_has_unwrapped_text = false;
    let mut extra_lines = 0;
    if label_type == "markdown" {
        // Markdown's paragraph gap and image-only line boxes are properties of its final DOM.
        // Read only that XHTML; reparsing authored Markdown here would bypass sanitizer policy.
        let mut reader = quick_xml::Reader::from_str(xhtml);
        let mut paragraphs = 0usize;
        let mut image_only_paragraphs = 0usize;
        let mut paragraph_images = 0usize;
        let mut paragraph_has_text = false;
        let mut paragraph_depth = 0usize;
        let mut element_depth = 0usize;
        loop {
            if measurer.cancellation_requested() {
                return crate::text::TextMetrics {
                    width: 0.0,
                    height: 0.0,
                    line_count: 0,
                };
            }
            match reader.read_event() {
                Ok(quick_xml::events::Event::Start(tag)) => {
                    element_depth += 1;
                    if tag.name().as_ref() == "p" {
                        if paragraph_depth == 0 {
                            paragraphs += 1;
                            paragraph_images = 0;
                            paragraph_has_text = false;
                        }
                        paragraph_depth += 1;
                    } else if paragraph_depth > 0 && tag.name().as_ref() == "img" {
                        paragraph_images += 1;
                    }
                }
                Ok(quick_xml::events::Event::Empty(tag))
                    if paragraph_depth > 0 && tag.name().as_ref() == "img" =>
                {
                    paragraph_images += 1;
                }
                Ok(quick_xml::events::Event::Text(text)) => {
                    let decoded =
                        merman_model::text::decode_html_entities_to_unicode(text.as_ref());
                    let has_text = !trim_html_collapsible_ascii_whitespace(&decoded).is_empty();
                    if paragraph_depth > 0 {
                        paragraph_has_text |= has_text;
                    }
                    if element_depth == 0 {
                        markdown_has_unwrapped_text |= has_text;
                    }
                }
                Ok(quick_xml::events::Event::GeneralRef(reference)) => {
                    let encoded = format!("&{};", reference.as_ref());
                    let decoded = merman_model::text::decode_html_entities_to_unicode(&encoded);
                    let has_text = !trim_html_collapsible_ascii_whitespace(&decoded).is_empty();
                    paragraph_has_text |= paragraph_depth > 0 && has_text;
                    markdown_has_unwrapped_text |= element_depth == 0 && has_text;
                }
                Ok(quick_xml::events::Event::End(tag)) => {
                    element_depth = element_depth.saturating_sub(1);
                    if tag.name().as_ref() == "p" {
                        paragraph_depth = paragraph_depth.saturating_sub(1);
                        if paragraph_depth == 0 && paragraph_images > 0 && !paragraph_has_text {
                            image_only_paragraphs += 1;
                        }
                    }
                }
                Ok(quick_xml::events::Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        extra_lines = if lower.contains("<img") {
            image_only_paragraphs
        } else {
            paragraphs.saturating_sub(1)
        };
    }
    let mut metrics = if lower.contains("<img") {
        // The generated image-only paragraph can own the fixed 80px width. A style
        // on surrounding text must never turn a mixed label into an image-only box.
        let inner = xhtml
            .strip_prefix("<p>")
            .and_then(|html| html.strip_suffix("</p>"))
            .unwrap_or(xhtml);
        let fixed_img_width = is_single_img_label(inner)
            && (lower.contains("min-width: 80px;") || label_type == "markdown");
        measure_flowchart_html_images(
            &request,
            xhtml,
            geometry,
            authored_width,
            fixed_img_width,
            probe_intrinsic_images,
        )
    } else if has_inline_style
        || flowchart_html_has_inline_style_tags(&lower)
        || lower.contains("<i ")
        || lower.contains("<br")
        || (label_type == "markdown" && !markdown_has_unwrapped_text)
    {
        measure_html_with_inline_styles(measurer, xhtml, style, max_width_px, WrapMode::HtmlLike)
    } else {
        let plain = flowchart_label_plain_text_for_layout(xhtml, "html", true);
        if plain.is_empty() {
            crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            }
        } else if plain.contains('<') {
            measure_html_with_inline_styles(
                measurer,
                xhtml,
                style,
                max_width_px,
                WrapMode::HtmlLike,
            )
        } else {
            measurer.measure_wrapped(&plain, style, max_width_px, WrapMode::HtmlLike)
        }
    };
    if label_type == "markdown"
        && !markdown_has_unwrapped_text
        && !lower.contains("<img")
        && !lower.contains("<i ")
        && let Some(max_width) = max_width_px.filter(|width| width.is_finite() && *width > 0.0)
        && metrics.width > max_width
        && !crate::text::mermaid_markdown_contains_html_tags(raw_label)
        && !crate::text::mermaid_markdown_contains_raw_blocks(raw_label)
    {
        // Markdown's min-content overflow keeps the host measurer's base width, rather than
        // snapping it to the ordinary browser lattice. Authored HTML and raw blocks retain
        // their rich-text widths. Source classification only selects this compatibility rule;
        // the measured text still comes exclusively from the sanitized final fragment.
        let plain = flowchart_label_plain_text_for_layout(xhtml, "html", true);
        if !plain.contains('<') {
            metrics.width = measurer
                .measure_wrapped(&plain, style, Some(max_width), WrapMode::HtmlLike)
                .width
                .max(max_width);
        }
    }
    metrics.height += extra_lines as f64 * style.font_size.max(1.0) * 1.5;
    metrics.line_count += extra_lines;
    metrics
}

fn measure_flowchart_html_images(
    request: &FlowchartLabelMetricsRequest<'_>,
    html: &str,
    geometry: HtmlTextGeometryDeclarations,
    authored_width: Option<HtmlLabelWidth>,
    fixed_img_width: bool,
    probe_intrinsic_images: bool,
) -> crate::text::TextMetrics {
    let FlowchartLabelMetricsRequest {
        measurer,
        style,
        max_width_px,
        ..
    } = *request;
    let wrap_width = authored_width.map_or(max_width_px, |width| width.wrap);
    let max_width = wrap_width.unwrap_or(200.0).max(1.0);
    let text_wrap_width = authored_width.map_or(Some(max_width), |width| width.wrap);
    let fallback_width = if fixed_img_width { 80.0 } else { max_width };
    let mut metrics = crate::text::TextMetrics {
        width: 0.0,
        height: 0.0,
        line_count: 0,
    };
    let mut text_buf = String::new();
    let mut height_per_px = 0.0;
    let mut reader = Reader::from_str(html);
    loop {
        if measurer.cancellation_requested() {
            break;
        }
        match reader.read_event() {
            Ok(Event::Start(tag) | Event::Empty(tag)) if tag.name().as_ref() == "img" => {
                measure_image_label_text(
                    &mut metrics,
                    &text_buf,
                    measurer,
                    style,
                    geometry,
                    text_wrap_width,
                );
                text_buf.clear();
                if let Some(height) = html_image_dimension(&tag, "height") {
                    // With width:100%, a presentational width does not size the mixed label.
                    // The authored height still controls the image's block contribution.
                    if fixed_img_width {
                        metrics.width = metrics.width.max(fallback_width);
                    }
                    metrics.height += height;
                    metrics.line_count += usize::from(height > 0.0);
                    continue;
                }
                let src = tag.try_get_attribute("src").ok().flatten();
                let src = src
                    .as_ref()
                    .and_then(|attr| attr.normalized_value(XmlVersion::Implicit1_0).ok());
                let src = src.as_deref().unwrap_or_default().trim();
                if let Some((natural_width, natural_height)) =
                    intrinsic_label_image_size(src, probe_intrinsic_images)
                {
                    let width = if fixed_img_width {
                        fallback_width
                    } else {
                        natural_width.min(max_width)
                    };
                    metrics.width = metrics.width.max(width);
                    // The image is fitted to the final label width after text is measured.
                    height_per_px += natural_height / natural_width;
                    metrics.line_count += 1;
                } else {
                    let width = if fixed_img_width {
                        fallback_width
                    } else {
                        html_image_dimension(&tag, "width").unwrap_or(fallback_width)
                    };
                    let height = if src.is_empty() { 0.0 } else { width };
                    metrics.width = metrics.width.max(width);
                    metrics.height += height;
                    metrics.line_count += usize::from(height > 0.0);
                }
            }
            Ok(Event::Start(tag) | Event::Empty(tag)) if tag.name().as_ref() == "br" => {
                text_buf.push('\n')
            }
            Ok(Event::Text(text)) => text_buf.push_str(text.as_ref()),
            Ok(Event::GeneralRef(reference)) => {
                text_buf.push('&');
                text_buf.push_str(reference.as_ref());
                text_buf.push(';');
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    measure_image_label_text(
        &mut metrics,
        &text_buf,
        measurer,
        style,
        geometry,
        text_wrap_width,
    );
    metrics.width = crate::text::ceil_to_1_64_px(metrics.width);
    let image_width = authored_width.map_or(metrics.width, |width| width.outer);
    metrics.height = crate::text::ceil_to_1_64_px(metrics.height + image_width * height_per_px);
    metrics
}

fn measure_image_label_text(
    metrics: &mut crate::text::TextMetrics,
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    geometry: HtmlTextGeometryDeclarations,
    wrap_width: Option<f64>,
) {
    if trim_html_collapsible_ascii_whitespace(text).is_empty() {
        return;
    }
    fn normalize_text_block(input: &str) -> String {
        // The scanner separates already-sanitized markup before measuring its DOM text. Decode
        // the XHTML representation once so entities do not become visible glyph spellings.
        let input = merman_model::text::decode_html_entities_to_unicode(input);
        let mut out = String::with_capacity(input.len());
        let mut last_space = false;
        for ch in input.chars() {
            if ch == '\n' {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push('\n');
                last_space = false;
                continue;
            }
            if is_html_collapsible_ascii_whitespace(ch) {
                if !last_space {
                    out.push(' ');
                }
                last_space = true;
                continue;
            }
            out.push(ch);
            last_space = false;
        }
        out.lines()
            .map(trim_html_collapsible_ascii_whitespace)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_matches(is_html_collapsible_ascii_whitespace)
            .to_string()
    }

    let text = normalize_text_block(text);
    if !text.is_empty() {
        let styled = (!geometry.is_empty())
            .then(|| {
                let fragment = crate::xml::escape_xml(&text).replace('\n', "<br />");
                measure_styled_html_label(measurer, &fragment, style, geometry, wrap_width, false)
            })
            .flatten();
        let text = styled.unwrap_or_else(|| {
            measurer.measure_wrapped(&text, style, wrap_width, WrapMode::HtmlLike)
        });
        metrics.width = metrics.width.max(text.width);
        metrics.height += text.height;
        metrics.line_count += text.line_count;
    }
}

fn html_image_dimension(tag: &BytesStart<'_>, name: &str) -> Option<f64> {
    let attr = tag.try_get_attribute(name).ok()??;
    let value = attr.normalized_value(XmlVersion::Implicit1_0).ok()?;
    value.trim().parse::<u32>().ok().map(f64::from)
}

fn intrinsic_label_image_size(src: &str, probe_intrinsic_images: bool) -> Option<(f64, f64)> {
    #[cfg(feature = "inline-raster-size")]
    if probe_intrinsic_images {
        return crate::svg_safety::intrinsic_raster_size(src).map(|size| (size.width, size.height));
    }
    let _ = (src, probe_intrinsic_images);
    None
}

fn flowchart_literal_paragraph_for_layout(text: &str) -> Cow<'_, str> {
    let text = flowchart_decode_label_escapes(text.trim_matches(' '));
    if !text.contains("  ") {
        return text;
    }

    let mut collapsed = String::with_capacity(text.len());
    for word in text.split(' ').filter(|word| !word.is_empty()) {
        if !collapsed.is_empty() {
            collapsed.push(' ');
        }
        collapsed.push_str(word);
    }
    Cow::Owned(collapsed)
}

pub fn flowchart_label_plain_text_for_layout(
    label: &str,
    label_type: &str,
    html_labels: bool,
) -> String {
    fn strip_html_for_layout(input: &str) -> String {
        // A lightweight, deterministic HTML text extractor for Mermaid htmlLabels layout.
        // We intentionally do not attempt full HTML parsing/sanitization here; we only need a
        // best-effort approximation of the rendered textContent for sizing.
        fn append_text_run(
            run: &mut String,
            out: &mut String,
            last_space: &mut bool,
            last_nl: &mut bool,
            trailing_newline_is_block_end: &mut bool,
        ) {
            if run.is_empty() {
                return;
            }
            {
                // Decode each DOM text run independently. Decoding after removing every tag would
                // incorrectly join `&cop<strong>y;</strong>` into the entity `&copy;`.
                let decoded = merman_model::text::decode_html_entities_to_unicode(run.as_str());
                for ch in decoded.chars() {
                    if ch == '\u{00A0}' {
                        out.push(ch);
                        *last_space = false;
                        *last_nl = false;
                        *trailing_newline_is_block_end = false;
                        continue;
                    }
                    if is_html_collapsible_ascii_whitespace(ch) {
                        if !*last_space && !*last_nl {
                            out.push(' ');
                            *last_space = true;
                            *trailing_newline_is_block_end = false;
                        }
                        continue;
                    }
                    out.push(ch);
                    *last_space = false;
                    *last_nl = false;
                    *trailing_newline_is_block_end = false;
                }
            }
            run.clear();
        }

        fn trim_trailing_inline_collapsible_space(out: &mut String) {
            while out
                .chars()
                .last()
                .is_some_and(|ch| matches!(ch, '\t' | '\u{000C}' | '\r' | ' '))
            {
                out.pop();
            }
        }

        fn push_br(
            out: &mut String,
            last_space: &mut bool,
            last_nl: &mut bool,
            trailing_newline_is_block_end: &mut bool,
        ) {
            trim_trailing_inline_collapsible_space(out);
            out.push('\n');
            *last_space = false;
            *last_nl = true;
            *trailing_newline_is_block_end = false;
        }

        fn push_block_end(
            out: &mut String,
            last_space: &mut bool,
            last_nl: &mut bool,
            trailing_newline_is_block_end: &mut bool,
        ) {
            trim_trailing_inline_collapsible_space(out);
            if !out.is_empty() && !*last_nl {
                out.push('\n');
                *trailing_newline_is_block_end = true;
            }
            *last_space = false;
            *last_nl = true;
        }

        let mut out = String::with_capacity(input.len());
        let mut text_run = String::new();
        let mut last_space = false;
        let mut last_nl = false;
        let mut trailing_newline_is_block_end = false;
        let mut it = input.chars().peekable();
        fn is_html_tag_start(ch: Option<char>) -> bool {
            ch.is_some_and(|ch| ch.is_ascii_alphabetic() || matches!(ch, '/' | '!' | '?'))
        }

        while let Some(ch) = it.next() {
            if ch == '<' {
                if !is_html_tag_start(it.peek().copied()) {
                    text_run.push('<');
                    continue;
                }

                append_text_run(
                    &mut text_run,
                    &mut out,
                    &mut last_space,
                    &mut last_nl,
                    &mut trailing_newline_is_block_end,
                );

                let mut tag = String::new();
                for c in it.by_ref() {
                    if c == '>' {
                        break;
                    }
                    tag.push(c);
                }
                let tag = trim_html_collapsible_ascii_whitespace(&tag);
                let tag_lower = tag.to_ascii_lowercase();
                let tag_trim = trim_html_collapsible_ascii_whitespace(&tag_lower);
                if tag_trim.starts_with('!') || tag_trim.starts_with('?') {
                    continue;
                }
                let is_closing = tag_trim.starts_with('/');
                let name = tag_trim
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .split(is_html_collapsible_ascii_whitespace)
                    .find(|part| !part.is_empty())
                    .unwrap_or("");
                if name == "br" {
                    push_br(
                        &mut out,
                        &mut last_space,
                        &mut last_nl,
                        &mut trailing_newline_is_block_end,
                    );
                } else if is_closing && matches!(name, "p" | "div" | "li" | "tr" | "ul" | "ol") {
                    push_block_end(
                        &mut out,
                        &mut last_space,
                        &mut last_nl,
                        &mut trailing_newline_is_block_end,
                    );
                }
                continue;
            }

            text_run.push(ch);
        }
        append_text_run(
            &mut text_run,
            &mut out,
            &mut last_space,
            &mut last_nl,
            &mut trailing_newline_is_block_end,
        );
        if trailing_newline_is_block_end {
            out.pop();
        }
        out
    }

    match label_type {
        "markdown" => {
            if !html_labels {
                return crate::text::mermaid_markdown_svg_plain_text(label);
            }

            if html_labels
                && (crate::text::mermaid_markdown_contains_raw_blocks(label)
                    || crate::text::mermaid_markdown_contains_html_tags(label))
            {
                let html = crate::text::mermaid_markdown_to_html_label_fragment(label, true);
                return trim_html_collapsible_ascii_whitespace(&strip_html_for_layout(&html))
                    .to_string();
            }

            let mut out = String::new();
            let parser = pulldown_cmark::Parser::new_ext(
                label,
                pulldown_cmark::Options::ENABLE_TABLES
                    | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                    | pulldown_cmark::Options::ENABLE_TASKLISTS,
            );
            for ev in parser {
                match ev {
                    pulldown_cmark::Event::Text(t) => out.push_str(&t),
                    pulldown_cmark::Event::Code(t) => out.push_str(&t),
                    pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                        out.push('\n');
                    }
                    _ => {}
                }
            }
            trim_html_collapsible_ascii_whitespace(&out).to_string()
        }
        _ => {
            if !html_labels {
                return flowchart_non_markdown_svg_plain_text(label);
            }

            let normalized = label.replace("\r\n", "\n");
            let text = flowchart_decode_label_escapes(&normalized);
            // Keep the raw label text for layout, then strip HTML tags/entities.
            //
            // Note: in Mermaid flowchart-v2, FontAwesome icon tokens (e.g. `fa:fa-car`)
            // can affect the measured label width even though the exported SVG replaces them
            // with empty `<i class="fa ..."></i>` nodes (FontAwesome CSS is not embedded).
            // For strict parity we therefore *do not* rewrite the `fa:` token here.
            strip_html_for_layout(&text)
                .trim_matches(|ch| matches!(ch, '\t' | '\u{000C}' | '\r' | ' '))
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_html_paragraph_preserves_layout_whitespace_and_escape_projection() {
        for text in ["", "   ", "A B", "  A   B  ", r#" A\\B  \: "C" > D "#] {
            let xhtml = format!("<p>{text}</p>");
            let Some(Cow::Borrowed(literal)) = crate::text::mermaid_xhtml_label_plain_text(&xhtml)
            else {
                panic!("expected a literal paragraph for {xhtml}");
            };
            assert_eq!(
                flowchart_literal_paragraph_for_layout(literal),
                flowchart_label_plain_text_for_layout(&xhtml, "html", true),
                "{xhtml}",
            );
        }
    }
}
