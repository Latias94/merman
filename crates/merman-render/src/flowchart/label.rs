use crate::math::MathRenderer;
use crate::model::{Bounds, LayoutEdge, LayoutNode};
#[cfg(test)]
use crate::text::TextMetrics;
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use merman_core::MermaidConfig;

pub(crate) struct FlowchartLabelMetricsRequest<'a> {
    pub(crate) measurer: &'a dyn TextMeasurer,
    pub(crate) raw_label: &'a str,
    pub(crate) label_type: &'a str,
    pub(crate) style: &'a TextStyle,
    pub(crate) max_width_px: Option<f64>,
    pub(crate) wrap_mode: WrapMode,
    pub(crate) config: &'a MermaidConfig,
    pub(crate) math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
}

fn is_html_collapsible_ascii_whitespace(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}

// Mermaid 11.16 inserts decoded labels through `addHtmlSpan(...).html(...)`. HTML collapses the
// ASCII whitespace set at a boundary, but U+00A0 remains visible and contributes to the line box.
pub(crate) fn flowchart_trim_html_collapsible_whitespace(input: &str) -> &str {
    input.trim_matches(is_html_collapsible_ascii_whitespace)
}

pub(crate) fn flowchart_label_text_is_empty_for_mode(text: &str, html_labels: bool) -> bool {
    if html_labels {
        flowchart_trim_html_collapsible_whitespace(text).is_empty()
    } else {
        text.trim().is_empty()
    }
}

pub(crate) fn flowchart_label_metrics_for_layout(
    req: FlowchartLabelMetricsRequest<'_>,
) -> crate::text::TextMetrics {
    let FlowchartLabelMetricsRequest {
        measurer,
        raw_label,
        label_type,
        style,
        max_width_px,
        wrap_mode,
        config,
        math_renderer,
    } = req;

    let math_metrics =
        crate::math::math_label_metrics_for_layout(crate::math::MathLabelMetricsRequest {
            measurer,
            raw_label,
            style,
            max_width_px,
            wrap_mode,
            config,
            math_renderer,
        });

    if let Some(m) = math_metrics {
        m
    } else if label_type == "markdown" {
        if wrap_mode != WrapMode::HtmlLike {
            // Mermaid 11.15 wraps SVG markdown node labels before reading the browser bbox.
            // Use the same wrapped word rows that the Flowchart SVG writer emits.
            crate::text::measure_wrapped_markdown_with_inline_styles(
                measurer,
                raw_label,
                style,
                max_width_px,
                wrap_mode,
            )
        } else {
            let has_raw_blocks = crate::text::mermaid_markdown_contains_raw_blocks(raw_label);
            let has_inline_html = crate::text::mermaid_markdown_contains_html_tags(raw_label);
            if (has_raw_blocks || has_inline_html) && !raw_label.contains("![") {
                let markdown_auto_wrap = config
                    .as_value()
                    .get("markdownAutoWrap")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true);
                let html = crate::text::mermaid_markdown_to_html_label_fragment(
                    raw_label,
                    markdown_auto_wrap,
                );
                let html = crate::text::replace_fontawesome_icons(&html);
                let plain = flowchart_label_plain_text_for_layout(raw_label, label_type, true);
                let has_inline_markup = html.contains("<strong>")
                    || html.contains("<em>")
                    || html.contains("<img")
                    || html.contains("<i ");
                if has_inline_html || has_inline_markup {
                    crate::text::measure_html_with_inline_styles(
                        measurer,
                        &html,
                        style,
                        max_width_px,
                        wrap_mode,
                    )
                } else {
                    measurer.measure_wrapped(&plain, style, max_width_px, wrap_mode)
                }
            } else {
                crate::text::measure_markdown_with_inline_styles(
                    measurer,
                    raw_label,
                    style,
                    max_width_px,
                    wrap_mode,
                )
            }
        }
    } else {
        let html_labels = wrap_mode == WrapMode::HtmlLike;
        if html_labels {
            fn measure_flowchart_html_images(
                measurer: &dyn TextMeasurer,
                html: &str,
                style: &TextStyle,
                max_width_px: Option<f64>,
            ) -> crate::text::TextMetrics {
                let max_width = max_width_px.unwrap_or(200.0).max(1.0);
                let lower = html.to_ascii_lowercase();
                if !lower.contains("<img") {
                    return measurer.measure_wrapped(html, style, max_width_px, WrapMode::HtmlLike);
                }

                fn has_img_src(tag: &str) -> bool {
                    let lower = tag.to_ascii_lowercase();
                    let Some(idx) = lower.find("src=") else {
                        return false;
                    };
                    let rest = tag[idx + 4..].trim_start();
                    let Some(quote) = rest.chars().next() else {
                        return false;
                    };
                    if quote != '"' && quote != '\'' {
                        return false;
                    }
                    let mut it = rest.chars();
                    let _ = it.next();
                    let mut val = String::new();
                    for ch in it {
                        if ch == quote {
                            break;
                        }
                        val.push(ch);
                    }
                    !val.trim().is_empty()
                }

                fn is_single_img_tag(html: &str) -> bool {
                    let t = flowchart_trim_html_collapsible_whitespace(html);
                    let lower = t.to_ascii_lowercase();
                    if !lower.starts_with("<img") {
                        return false;
                    }
                    let Some(end) = t.find('>') else {
                        return false;
                    };
                    flowchart_trim_html_collapsible_whitespace(&t[end + 1..]).is_empty()
                }

                let fixed_img_width = is_single_img_tag(html);
                let img_w = if fixed_img_width { 80.0 } else { max_width };

                if fixed_img_width {
                    let img_h = if has_img_src(html) { img_w } else { 0.0 };
                    return crate::text::TextMetrics {
                        width: crate::text::ceil_to_1_64_px(img_w),
                        height: crate::text::ceil_to_1_64_px(img_h),
                        line_count: if img_h > 0.0 { 1 } else { 0 },
                    };
                }

                #[derive(Debug, Clone)]
                enum Block {
                    Text(String),
                    Img { has_src: bool },
                }

                let mut blocks: Vec<Block> = Vec::new();
                let mut text_buf = String::new();

                let bytes = html.as_bytes();
                let mut i = 0usize;
                while i < bytes.len() {
                    if bytes[i] == b'<' {
                        let rest = &html[i..];
                        let rest_lower = rest.to_ascii_lowercase();
                        if rest_lower.starts_with("<img")
                            && let Some(rel_end) = rest.find('>')
                        {
                            if !flowchart_trim_html_collapsible_whitespace(&text_buf).is_empty() {
                                blocks.push(Block::Text(std::mem::take(&mut text_buf)));
                            } else {
                                text_buf.clear();
                            }
                            let tag = &rest[..=rel_end];
                            blocks.push(Block::Img {
                                has_src: has_img_src(tag),
                            });
                            i += rel_end + 1;
                            continue;
                        }
                        if rest_lower.starts_with("<br")
                            && let Some(rel_end) = rest.find('>')
                        {
                            text_buf.push('\n');
                            i += rel_end + 1;
                            continue;
                        }
                        if let Some(rel_end) = rest.find('>') {
                            i += rel_end + 1;
                            continue;
                        }
                    }
                    let Some(ch) = html[i..].chars().next() else {
                        break;
                    };
                    text_buf.push(ch);
                    i += ch.len_utf8();
                }
                if !flowchart_trim_html_collapsible_whitespace(&text_buf).is_empty() {
                    blocks.push(Block::Text(text_buf));
                }

                fn normalize_text_block(input: &str) -> String {
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
                        .map(flowchart_trim_html_collapsible_whitespace)
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim_matches(is_html_collapsible_ascii_whitespace)
                        .to_string()
                }

                let mut width: f64 = 0.0;
                let mut height: f64 = 0.0;
                let mut lines = 0usize;

                for b in blocks {
                    match b {
                        Block::Img { has_src } => {
                            width = width.max(img_w);
                            let img_h = if has_src { img_w } else { 0.0 };
                            height += img_h;
                            if img_h > 0.0 {
                                lines += 1;
                            }
                        }
                        Block::Text(t) => {
                            let t = normalize_text_block(&t);
                            if t.is_empty() {
                                continue;
                            }
                            let m = measurer.measure_wrapped(
                                &t,
                                style,
                                Some(max_width),
                                WrapMode::HtmlLike,
                            );
                            width = width.max(m.width);
                            height += m.height;
                            lines += m.line_count;
                        }
                    }
                }

                crate::text::TextMetrics {
                    width: crate::text::ceil_to_1_64_px(width),
                    height: crate::text::ceil_to_1_64_px(height),
                    line_count: lines,
                }
            }

            let mut label = raw_label.replace("\r\n", "\n");
            if label_type == "string" {
                label = flowchart_trim_html_collapsible_whitespace(&label).to_string();
            }
            let label = label.trim_end_matches('\n');
            let wants_p = crate::text::mermaid_markdown_wants_paragraph_wrap(label);
            let label = if wants_p {
                label.replace('\n', "<br />")
            } else {
                label.to_string()
            };
            let fixed_img_width = {
                let t = flowchart_trim_html_collapsible_whitespace(&label);
                let lower = t.to_ascii_lowercase();
                lower.starts_with("<img")
                    && t.find('>').is_some_and(|end| {
                        flowchart_trim_html_collapsible_whitespace(&t[end + 1..]).is_empty()
                    })
            };
            let html = if fixed_img_width || !wants_p {
                label
            } else {
                format!("<p>{}</p>", label)
            };
            let html = crate::text::replace_fontawesome_icons(&html);

            let lower = html.to_ascii_lowercase();
            let has_inline_style = crate::text::flowchart_html_has_inline_style_tags(&lower);

            if lower.contains("<img") {
                measure_flowchart_html_images(measurer, &html, style, max_width_px)
            } else if has_inline_style || html.contains("<i ") {
                crate::text::measure_html_with_inline_styles(
                    measurer,
                    &html,
                    style,
                    max_width_px,
                    wrap_mode,
                )
            } else {
                let label_for_metrics =
                    flowchart_label_plain_text_for_layout(raw_label, label_type, html_labels);
                measurer.measure_wrapped(&label_for_metrics, style, max_width_px, wrap_mode)
            }
        } else {
            let label_for_metrics =
                flowchart_label_plain_text_for_layout(raw_label, label_type, html_labels);
            measurer.measure_wrapped(&label_for_metrics, style, max_width_px, wrap_mode)
        }
    }
}

pub(crate) fn flowchart_decode_label_escapes(label: &str) -> String {
    if !label.contains('\\') {
        return label.to_string();
    }

    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('\\') => {
                out.push('\\');
                chars.next();
            }
            Some(':') => {
                out.push(':');
                chars.next();
            }
            _ => out.push('\\'),
        }
    }
    out
}

pub(crate) fn flowchart_normalize_plain_multiline_label_for_html(
    label: &str,
) -> std::borrow::Cow<'_, str> {
    if !label.contains('\n') {
        return std::borrow::Cow::Borrowed(label);
    }

    std::borrow::Cow::Owned(
        label
            .split('\n')
            .map(flowchart_trim_html_collapsible_whitespace)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_matches(is_html_collapsible_ascii_whitespace)
            .to_string(),
    )
}

pub(crate) fn flowchart_label_plain_text_for_layout(
    label: &str,
    label_type: &str,
    html_labels: bool,
) -> String {
    fn decode_html_entity(entity: &str) -> Option<char> {
        match entity {
            "nbsp" => Some('\u{00A0}'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "#39" => Some('\''),
            _ => {
                if let Some(hex) = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>().ok().and_then(char::from_u32)
                } else {
                    None
                }
            }
        }
    }

    fn strip_html_for_layout(input: &str) -> String {
        // A lightweight, deterministic HTML text extractor for Mermaid htmlLabels layout.
        // We intentionally do not attempt full HTML parsing/sanitization here; we only need a
        // best-effort approximation of the rendered textContent for sizing.
        fn trim_trailing_break_whitespace(out: &mut String) {
            loop {
                let Some(ch) = out.chars().last() else {
                    return;
                };
                if ch == '\n' {
                    return;
                }
                if is_html_collapsible_ascii_whitespace(ch) {
                    out.pop();
                    continue;
                }
                return;
            }
        }

        let mut out = String::with_capacity(input.len());
        let mut it = input.chars().peekable();
        fn is_html_tag_start(ch: Option<char>) -> bool {
            ch.is_some_and(|ch| ch.is_ascii_alphabetic() || matches!(ch, '/' | '!' | '?'))
        }

        while let Some(ch) = it.next() {
            if ch == '<' {
                if !is_html_tag_start(it.peek().copied()) {
                    out.push('<');
                    continue;
                }

                let mut tag = String::new();
                for c in it.by_ref() {
                    if c == '>' {
                        break;
                    }
                    tag.push(c);
                }
                let tag = tag.trim();
                let tag_lower = tag.to_ascii_lowercase();
                let tag_trim = tag_lower.trim();
                if tag_trim.starts_with('!') || tag_trim.starts_with('?') {
                    continue;
                }
                let is_closing = tag_trim.starts_with('/');
                let name = tag_trim
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if name == "br"
                    || (is_closing && matches!(name, "p" | "div" | "li" | "tr" | "ul" | "ol"))
                {
                    trim_trailing_break_whitespace(&mut out);
                    out.push('\n');
                }
                continue;
            }

            if ch == '&' {
                let mut entity = String::new();
                let mut saw_semicolon = false;
                while let Some(&c) = it.peek() {
                    if c == ';' {
                        it.next();
                        saw_semicolon = true;
                        break;
                    }
                    if c == '<' || c == '&' || c.is_whitespace() || entity.len() > 32 {
                        break;
                    }
                    entity.push(c);
                    it.next();
                }
                if saw_semicolon {
                    if let Some(decoded) = decode_html_entity(entity.as_str()) {
                        out.push(decoded);
                    } else {
                        out.push('&');
                        out.push_str(&entity);
                        out.push(';');
                    }
                } else {
                    out.push('&');
                    out.push_str(&entity);
                }
                continue;
            }

            out.push(ch);
        }

        // Collapse whitespace runs similar to HTML layout defaults, while preserving explicit
        // line breaks introduced by tags like `<br>` and `</p>`.
        let mut normalized = String::with_capacity(out.len());
        let mut last_space = false;
        let mut last_nl = false;
        for ch in out.chars() {
            if ch == '\u{00A0}' {
                normalized.push(ch);
                last_space = false;
                last_nl = false;
                continue;
            }
            if ch == '\n' {
                if !last_nl {
                    normalized.push('\n');
                }
                last_space = false;
                last_nl = true;
                continue;
            }
            if is_html_collapsible_ascii_whitespace(ch) {
                if !last_space && !last_nl {
                    normalized.push(' ');
                    last_space = true;
                }
                continue;
            }
            normalized.push(ch);
            last_space = false;
            last_nl = false;
        }

        normalized
    }

    match label_type {
        "markdown" => {
            if html_labels
                && (crate::text::mermaid_markdown_contains_raw_blocks(label)
                    || crate::text::mermaid_markdown_contains_html_tags(label))
            {
                let html = crate::text::mermaid_markdown_to_html_label_fragment(label, true);
                return flowchart_trim_html_collapsible_whitespace(&strip_html_for_layout(&html))
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
            if html_labels {
                flowchart_trim_html_collapsible_whitespace(&out).to_string()
            } else {
                out.trim().to_string()
            }
        }
        _ => {
            let mut t = flowchart_decode_label_escapes(&label.replace("\r\n", "\n"));
            if html_labels || label_type == "html" {
                // Keep the raw label text for layout, then strip HTML tags/entities.
                //
                // Note: in Mermaid flowchart-v2, FontAwesome icon tokens (e.g. `fa:fa-car`)
                // can affect the measured label width even though the exported SVG replaces them
                // with empty `<i class="fa ..."></i>` nodes (FontAwesome CSS is not embedded).
                // For strict parity we therefore *do not* rewrite the `fa:` token here.
                t = strip_html_for_layout(&t);
            } else {
                t = t.replace("<br />", "\n");
                t = t.replace("<br/>", "\n");
                t = t.replace("<br>", "\n");
                t = t.replace("</br>", "\n");
                t = t.replace("</br/>", "\n");
                t = t.replace("</br />", "\n");
                t = t.replace("</br >", "\n");

                // In SVG-label mode (htmlLabels=false), Mermaid renders `<tag>text</tag>` as
                // escaped literal tag tokens with whitespace separation (see
                // `upstream_flowchart_v2_escaped_without_html_labels_spec`).
                //
                // For layout measurement we approximate that by inserting spaces between
                // adjacent tag/text tokens when the source omits them.
                fn space_separate_html_like_tags_for_svg_labels(input: &str) -> String {
                    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                    enum TokKind {
                        Text,
                        Tag,
                        Newline,
                    }

                    fn is_tag_start(s: &str) -> bool {
                        let mut it = s.chars();
                        if it.next() != Some('<') {
                            return false;
                        }
                        let Some(next) = it.next() else {
                            return false;
                        };
                        next.is_ascii_alphabetic() || matches!(next, '/' | '!' | '?')
                    }

                    let mut out = String::with_capacity(input.len());
                    let mut prev_kind: Option<TokKind> = None;

                    let mut i = 0usize;
                    while i < input.len() {
                        let rest = &input[i..];
                        if rest.starts_with('\n') {
                            out.push('\n');
                            prev_kind = Some(TokKind::Newline);
                            i += 1;
                            continue;
                        }

                        if is_tag_start(rest) {
                            let Some(rel_end) = rest.find('>') else {
                                // Malformed tag; treat as text.
                                let Some(ch) = rest.chars().next() else {
                                    break;
                                };
                                out.push(ch);
                                prev_kind = Some(TokKind::Text);
                                i += ch.len_utf8();
                                continue;
                            };

                            let tag = &rest[..=rel_end];
                            if matches!(prev_kind, Some(TokKind::Text))
                                && !out.ends_with(|ch: char| ch.is_whitespace())
                            {
                                out.push(' ');
                            }
                            out.push_str(tag);
                            prev_kind = Some(TokKind::Tag);
                            i += rel_end + 1;
                            continue;
                        }

                        // Text run until next newline or tag start.
                        let mut run_end = input.len();
                        if let Some(nl) = rest.find('\n') {
                            run_end = run_end.min(i + nl);
                        }
                        if let Some(lt) = rest.find('<') {
                            run_end = run_end.min(i + lt);
                        }
                        let run = &input[i..run_end];
                        if matches!(prev_kind, Some(TokKind::Tag))
                            && !run.starts_with(|ch: char| ch.is_whitespace())
                        {
                            out.push(' ');
                        }
                        out.push_str(run);
                        prev_kind = Some(TokKind::Text);
                        i = run_end;
                    }

                    out
                }

                t = space_separate_html_like_tags_for_svg_labels(&t);
            }
            if html_labels {
                flowchart_trim_html_collapsible_whitespace(&t).to_string()
            } else {
                t.trim().trim_end_matches('\n').to_string()
            }
        }
    }
}

fn compute_bounds_impl<E>(
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    mut charge: impl FnMut(usize) -> std::result::Result<(), E>,
) -> std::result::Result<Option<Bounds>, E> {
    fn include(bounds: &mut Option<Bounds>, x: f64, y: f64) {
        let Some(bounds) = bounds.as_mut() else {
            *bounds = Some(Bounds {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            });
            return;
        };
        bounds.min_x = bounds.min_x.min(x);
        bounds.min_y = bounds.min_y.min(y);
        bounds.max_x = bounds.max_x.max(x);
        bounds.max_y = bounds.max_y.max(y);
    }

    let mut bounds = None;
    for n in nodes {
        charge(2)?;
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        include(&mut bounds, n.x - hw, n.y - hh);
        include(&mut bounds, n.x + hw, n.y + hh);
    }
    for e in edges {
        charge(1)?;
        charge(e.points.len())?;
        for p in &e.points {
            include(&mut bounds, p.x, p.y);
        }
        if let Some(l) = &e.label {
            charge(2)?;
            let hw = l.width / 2.0;
            let hh = l.height / 2.0;
            include(&mut bounds, l.x - hw, l.y - hh);
            include(&mut bounds, l.x + hw, l.y + hh);
        }
    }
    Ok(bounds)
}

#[cfg(feature = "layout-elk")]
pub(super) fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    match compute_bounds_impl(nodes, edges, |_| Ok::<(), std::convert::Infallible>(())) {
        Ok(bounds) => bounds,
        Err(never) => match never {},
    }
}

pub(super) fn compute_bounds_controlled(
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    charge: impl FnMut(usize) -> crate::Result<()>,
) -> crate::Result<Option<Bounds>> {
    compute_bounds_impl(nodes, edges, charge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::MathRenderer;
    use crate::model::{LayoutLabel, LayoutPoint};

    #[derive(Debug)]
    struct PreciseMathRenderer;

    impl MathRenderer for PreciseMathRenderer {
        fn render_html_label(&self, text: &str, _config: &MermaidConfig) -> Option<String> {
            text.contains("$$").then(|| text.to_string())
        }

        fn measure_html_label(
            &self,
            text: &str,
            _config: &MermaidConfig,
            _style: &TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> Option<TextMetrics> {
            (text.starts_with("$$") && text.ends_with("$$")).then_some(TextMetrics {
                width: 10.008,
                height: 20.008,
                line_count: 1,
            })
        }
    }

    struct PreciseTextMeasurer;

    impl TextMeasurer for PreciseTextMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 1.001,
                height: 2.002,
                line_count: 1,
            }
        }
    }

    #[test]
    fn mixed_math_metrics_preserve_fragment_precision() {
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &PreciseTextMeasurer,
            raw_label: "a$$x$$b",
            label_type: "text",
            style: &style,
            max_width_px: None,
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: Some(&PreciseMathRenderer),
        });

        assert!((metrics.width - 12.01).abs() < 1e-12, "{metrics:?}");
        assert!((metrics.height - 20.008).abs() < 1e-12, "{metrics:?}");
    }

    #[test]
    fn html_label_metrics_keep_nbsp_only_trailing_lines() {
        let config = MermaidConfig::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let measurer = crate::text::VendoredFontMetricsTextMeasurer::default();

        for (raw_label, label_type) in [("A<br>&nbsp;", "string"), ("A\n\u{00A0}", "markdown")] {
            let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type,
                style: &style,
                max_width_px: None,
                wrap_mode: WrapMode::HtmlLike,
                config: &config,
                math_renderer: None,
            });

            assert_eq!(metrics.line_count, 2, "{raw_label:?}: {metrics:?}");
            assert_eq!(metrics.height, 48.0, "{raw_label:?}: {metrics:?}");
        }
    }

    #[test]
    fn controlled_bounds_streams_geometry_and_charges_each_visited_item() {
        let nodes = vec![LayoutNode {
            id: "node".to_string(),
            x: 10.0,
            y: 20.0,
            width: 8.0,
            height: 6.0,
            is_cluster: false,
            label_width: None,
            label_height: None,
        }];
        let edges = vec![LayoutEdge {
            id: "edge".to_string(),
            from: "node".to_string(),
            to: "node".to_string(),
            from_cluster: None,
            to_cluster: None,
            points: vec![
                LayoutPoint { x: -5.0, y: 1.0 },
                LayoutPoint { x: 7.0, y: 30.0 },
                LayoutPoint { x: 14.0, y: 18.0 },
            ],
            label: Some(LayoutLabel {
                x: 4.0,
                y: 5.0,
                width: 4.0,
                height: 2.0,
            }),
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        }];
        let mut tranches = Vec::new();

        let bounds = compute_bounds_controlled(&nodes, &edges, |units| {
            tranches.push(units);
            Ok(())
        })
        .expect("the accounting callback accepts every tranche")
        .expect("the geometry is non-empty");

        assert_eq!(tranches, vec![2, 1, 3, 2]);
        assert_eq!(tranches.iter().sum::<usize>(), 8);
        assert_eq!(
            bounds,
            Bounds {
                min_x: -5.0,
                min_y: 1.0,
                max_x: 14.0,
                max_y: 30.0,
            }
        );
    }
}
