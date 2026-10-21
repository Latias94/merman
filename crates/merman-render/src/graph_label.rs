use crate::math::MathRenderer;
use crate::model::{Bounds, LayoutEdge, LayoutNode};
#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
use crate::text::TextMetrics;
use crate::text::{
    TextMeasurer, TextStyle, WrapMode, is_ecmascript_whitespace,
    is_html_collapsible_ascii_whitespace, trim_html_collapsible_ascii_whitespace,
};
use merman_core::presentation::PresentationConfig;
use std::fmt::Write as _;
use unicode_segmentation::UnicodeSegmentation;

pub(crate) struct FlowchartLabelMetricsRequest<'a> {
    pub(crate) measurer: &'a dyn TextMeasurer,
    pub(crate) raw_label: &'a str,
    pub(crate) label_type: &'a str,
    pub(crate) style: &'a TextStyle,
    pub(crate) max_width_px: Option<f64>,
    pub(crate) wrap_mode: WrapMode,
    pub(crate) config: &'a PresentationConfig,
    pub(crate) math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
}

/// The FlowDB minimum only reaches ordinary nodes that use `labelHelper`.
/// Asset shapes set `node.width` before calling it; fixed-size symbols and label-derived
/// circles opt out of it.
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn flowchart_node_label_min_width(
    raw_label: &str,
    layout_shape: Option<&str>,
    config: &PresentationConfig,
) -> f64 {
    use crate::flowchart::FlowchartShape;

    let shape = FlowchartShape::resolve(layout_shape.unwrap_or("squareRect"));
    if raw_label.is_empty()
        || matches!(
            shape,
            Ok(FlowchartShape::Anchor
                | FlowchartShape::Circle
                | FlowchartShape::Choice
                | FlowchartShape::CrossedCircle
                | FlowchartShape::DoubleCircle
                | FlowchartShape::FilledCircle
                | FlowchartShape::ForkJoin
                | FlowchartShape::FramedCircle
                | FlowchartShape::SmallCircle
                | FlowchartShape::LightningBolt
                | FlowchartShape::Icon
                | FlowchartShape::IconCircle
                | FlowchartShape::IconRounded
                | FlowchartShape::IconSquare
                | FlowchartShape::ImageSquare
                | FlowchartShape::CollapsedGroup)
        )
    {
        return 0.0;
    }
    crate::config::config_f64(config.as_value(), &["flowchart", "minNodeWidth"])
        .unwrap_or(0.0)
        .max(0.0)
}

fn normalize_flowchart_svg_line_breaks(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pos = 0usize;
    while pos < input.len() {
        let rest = &input[pos..];
        if rest.starts_with("\\n") {
            out.push('\n');
            pos += 2;
            continue;
        }

        if rest.starts_with('<') && rest.len() >= 3 {
            let mut cursor = 1usize;
            if rest[cursor..].starts_with('/') {
                cursor += 1;
            }
            let mut chars = rest[cursor..].char_indices();
            if let Some((_, b)) = chars.next()
                && let Some((r_offset, r)) = chars.next()
                && b.eq_ignore_ascii_case(&'b')
                && r.eq_ignore_ascii_case(&'r')
            {
                let mut end = cursor + r_offset + r.len_utf8();
                while end < rest.len() {
                    let ch = rest[end..]
                        .chars()
                        .next()
                        .expect("position before input end has a character");
                    if !is_ecmascript_whitespace(ch) {
                        break;
                    }
                    end += ch.len_utf8();
                }
                if rest[end..].starts_with('/') {
                    end += 1;
                }
                if rest[end..].starts_with('>') {
                    out.push('\n');
                    pos += end + 1;
                    continue;
                }
            }
        }

        let ch = rest
            .chars()
            .next()
            .expect("position before input end has a character");
        out.push(ch);
        pos += ch.len_utf8();
    }
    out
}

/// Preserves Mermaid `nonMarkdownToLines()` word provenance for SVG labels.
///
/// Tokenization intentionally runs before the sanitizer entities are decoded. A literal
/// `<span class="a b">` is therefore one source word, while an entity-authored
/// `&lt;span class="a b"&gt;` keeps the ordinary whitespace boundaries it had in the source.
pub(crate) fn flowchart_non_markdown_svg_source_word_lines(label: &str) -> Vec<Vec<String>> {
    let normalized = label.replace("\r\n", "\n");
    let normalized = flowchart_decode_label_escapes(&normalized);
    let normalized = normalize_flowchart_svg_line_breaks(&normalized);

    normalized
        .split('\n')
        .map(|line| {
            crate::text::non_markdown_svg_words(line)
                .map(str::to_string)
                .collect()
        })
        .collect()
}

pub(crate) fn flowchart_svg_source_word_lines_plain_text(lines: &[Vec<String>]) -> String {
    let capacity = lines
        .iter()
        .flatten()
        .map(String::len)
        .sum::<usize>()
        .saturating_add(
            lines
                .iter()
                .map(|line| line.len().saturating_sub(1))
                .sum::<usize>(),
        )
        .saturating_add(lines.len().saturating_sub(1));
    let mut out = String::with_capacity(capacity);
    for (line_index, line) in lines.iter().enumerate() {
        if line_index > 0 {
            out.push('\n');
        }
        for (word_index, word) in line.iter().enumerate() {
            if word_index > 0 {
                out.push(' ');
            }
            let visible = crate::entities::decode_svg_text_content_entities(word);
            out.push_str(visible.as_ref());
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FlowchartSvgWidthMode {
    Bbox,
    ComputedLength,
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) fn flowchart_node_svg_width_mode(
    raw_label: &str,
    label_type: &str,
    wrap_mode: WrapMode,
    layout_shape: &str,
) -> FlowchartSvgWidthMode {
    if wrap_mode == WrapMode::SvgLike
        && label_type != "markdown"
        && !raw_label.contains('<')
        && !raw_label.contains('>')
        && crate::flowchart::is_flowchart_process_shape(layout_shape)
    {
        FlowchartSvgWidthMode::ComputedLength
    } else {
        FlowchartSvgWidthMode::Bbox
    }
}

/// Pure source projection of Mermaid's non-Markdown SVG createText payload.
///
/// The source rows retain authored entity and tag boundaries while `plain_text` is the exact
/// visible projection used for emptiness and metrics. This state is independent of style, width,
/// and text-measurement routing; measured preparation lives in the private Flowchart sidecar.
#[derive(Debug, Clone)]
pub(crate) struct FlowchartSvgLabelSource {
    source_lines: Vec<Vec<String>>,
    plain_text: String,
}

fn flowchart_source_plain_projection(source_lines: &[Vec<String>]) -> String {
    let mut plain_text = String::new();
    for (line_index, line) in source_lines.iter().enumerate() {
        if line_index > 0 {
            plain_text.push('\n');
        }
        for (word_index, source_word) in line.iter().enumerate() {
            if word_index > 0 {
                plain_text.push(' ');
            }
            let mut rest = source_word.as_str();
            while !rest.is_empty() {
                if let Some(entity_len) = authored_entity_len(rest) {
                    let entity = &rest[..entity_len];
                    plain_text.push_str(match entity {
                        "&lt;" => "<",
                        "&gt;" => ">",
                        "&amp;" => "&",
                        _ => entity,
                    });
                    rest = &rest[entity_len..];
                } else {
                    let character = rest.chars().next().expect("nonempty source word");
                    plain_text.push(character);
                    rest = &rest[character.len_utf8()..];
                }
            }
        }
    }
    plain_text
}

fn authored_entity_len(text: &str) -> Option<usize> {
    const MAX_AUTHORED_ENTITY_BYTES: usize = 64;

    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'&') {
        return None;
    }
    let scan = &bytes[..bytes.len().min(MAX_AUTHORED_ENTITY_BYTES)];
    let end = scan.iter().position(|byte| *byte == b';')?.checked_add(1)?;
    let body = text.get(1..end - 1)?;
    (!body.is_empty() && body.chars().all(|character| !character.is_whitespace())).then_some(end)
}

impl FlowchartSvgLabelSource {
    pub(crate) fn new(source: &str) -> Self {
        let source_lines = flowchart_non_markdown_svg_source_word_lines(source);
        let plain_text = flowchart_source_plain_projection(&source_lines);
        Self {
            source_lines,
            plain_text,
        }
    }

    pub(crate) fn plain_text(&self) -> &str {
        &self.plain_text
    }

    pub(crate) fn wrapped_lines(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
    ) -> Vec<Vec<String>> {
        flowchart_wrap_svg_source_word_lines(
            measurer,
            &self.source_lines,
            style,
            max_width_px,
            break_long_words,
        )
    }

    pub(crate) fn metrics(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        max_width_px: Option<f64>,
        width_mode: FlowchartSvgWidthMode,
    ) -> crate::text::TextMetrics {
        if measurer.cancellation_requested() {
            return crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            };
        }
        let wrapped = self.wrapped_lines(measurer, style, max_width_px, true);
        self.metrics_from_wrapped(measurer, style, &wrapped, width_mode)
    }

    pub(crate) fn metrics_from_wrapped(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        wrapped: &[Vec<String>],
        width_mode: FlowchartSvgWidthMode,
    ) -> crate::text::TextMetrics {
        if measurer.cancellation_requested() {
            return crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            };
        }
        if flowchart_label_text_is_empty_for_mode(&self.plain_text, false) {
            return crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            };
        }

        let visible = flowchart_svg_source_word_lines_plain_text(wrapped);
        let mut metrics = measurer.measure_wrapped(&visible, style, None, WrapMode::SvgLike);
        if width_mode == FlowchartSvgWidthMode::ComputedLength {
            metrics.width = wrapped.iter().fold(0.0_f64, |width, line| {
                let visible =
                    flowchart_svg_source_word_lines_plain_text(std::slice::from_ref(line));
                width.max(measurer.measure_svg_text_computed_length_px(&visible, style))
            });
        }
        metrics
    }
}

pub(crate) fn flowchart_wrap_svg_source_word_lines(
    measurer: &dyn TextMeasurer,
    lines: &[Vec<String>],
    style: &TextStyle,
    max_width_px: Option<f64>,
    break_long_words: bool,
) -> Vec<Vec<String>> {
    #[derive(Clone)]
    struct ProbeCheckpoint {
        builtin: Option<crate::environment::BuiltinSvgComputedLength>,
        visible_len: usize,
    }

    struct ComputedLengthProbe<'a> {
        measurer: &'a dyn TextMeasurer,
        style: &'a TextStyle,
        builtin: Option<crate::environment::BuiltinSvgComputedLength>,
        visible: String,
    }

    impl<'a> ComputedLengthProbe<'a> {
        fn new(measurer: &'a dyn TextMeasurer, style: &'a TextStyle) -> Self {
            Self {
                measurer,
                style,
                builtin: measurer.begin_svg_text_computed_length(style),
                visible: String::new(),
            }
        }

        fn has_builtin(&self) -> bool {
            self.builtin.is_some()
        }

        fn reset(&mut self) {
            if let Some(builtin) = self.builtin.as_mut() {
                builtin.reset();
            }
            self.visible.clear();
        }

        fn checkpoint(&self) -> ProbeCheckpoint {
            ProbeCheckpoint {
                builtin: self.builtin.clone(),
                visible_len: self.visible.len(),
            }
        }

        fn restore(&mut self, checkpoint: ProbeCheckpoint) {
            self.builtin = checkpoint.builtin;
            self.visible.truncate(checkpoint.visible_len);
        }

        fn push_visible(&mut self, text: &str) {
            if let Some(builtin) = self.builtin.as_mut() {
                builtin.push_text(text);
            } else {
                self.visible.push_str(text);
            }
        }

        fn push_source_word(&mut self, word: &str, prepend_space: bool) {
            if prepend_space {
                self.push_visible(" ");
            }
            let visible = crate::entities::decode_svg_text_content_entities(word);
            self.push_visible(visible.as_ref());
        }

        fn width_px(&self) -> f64 {
            self.builtin.as_ref().map_or_else(
                || {
                    self.measurer
                        .measure_svg_text_computed_length_px(&self.visible, self.style)
                },
                crate::environment::BuiltinSvgComputedLength::width_px,
            )
        }

        fn measure_source(&mut self, source: &str) -> f64 {
            self.reset();
            let visible = crate::entities::decode_svg_text_content_entities(source);
            self.push_visible(visible.as_ref());
            self.width_px()
        }
    }

    fn split_source_word_to_fit(
        probe: &mut ComputedLengthProbe<'_>,
        word: &str,
        max_width_px: f64,
    ) -> Vec<String> {
        let boundaries = word
            .grapheme_indices(true)
            .map(|(offset, _)| offset)
            .chain(std::iter::once(word.len()))
            .collect::<Vec<_>>();
        if boundaries.len() <= 1 {
            return vec![word.to_string()];
        }

        let has_decoded_entities =
            word.contains("&lt;") || word.contains("&gt;") || word.contains("&amp;");
        if probe.has_builtin() && !has_decoded_entities {
            let mut chunks = Vec::new();
            let mut start_index = 0usize;
            while start_index + 1 < boundaries.len() {
                if probe.measurer.cancellation_requested() {
                    return Vec::new();
                }
                probe.reset();
                let mut split_index = start_index + 1;
                let mut previous_end = boundaries[start_index];
                for (end_index, &end) in boundaries.iter().enumerate().skip(start_index + 1) {
                    if probe.measurer.cancellation_requested() {
                        return Vec::new();
                    }
                    probe.push_visible(&word[previous_end..end]);
                    if probe.width_px() <= max_width_px {
                        split_index = end_index;
                        previous_end = end;
                    } else {
                        break;
                    }
                }
                chunks.push(word[boundaries[start_index]..boundaries[split_index]].to_string());
                start_index = split_index;
            }
            return chunks;
        }

        // Opaque/custom measurers retain Mermaid's exact growing-prefix request order. The
        // grapheme boundaries are collected once and output chunks borrow the original word, so
        // the compatibility path no longer recopies/resegments every shrinking tail.
        let mut chunks = Vec::new();
        let mut start_index = 0usize;
        let mut first_tail_already_measured = true;
        while start_index + 1 < boundaries.len() {
            if probe.measurer.cancellation_requested() {
                return Vec::new();
            }
            let start = boundaries[start_index];
            if !first_tail_already_measured && probe.measure_source(&word[start..]) <= max_width_px
            {
                chunks.push(word[start..].to_string());
                break;
            }
            first_tail_already_measured = false;

            let mut split_index = start_index + 1;
            for (end_index, &end) in boundaries.iter().enumerate().skip(start_index + 1) {
                if probe.measurer.cancellation_requested() {
                    return Vec::new();
                }
                if probe.measure_source(&word[start..end]) <= max_width_px {
                    split_index = end_index;
                } else {
                    break;
                }
            }
            chunks.push(word[start..boundaries[split_index]].to_string());
            start_index = split_index;
        }
        chunks
    }

    let Some(max_width_px) = max_width_px.filter(|width| width.is_finite() && *width > 0.0) else {
        if measurer.cancellation_requested() {
            return Vec::new();
        }
        return lines.to_vec();
    };

    let mut wrapped = Vec::new();
    let mut probe = ComputedLengthProbe::new(measurer, style);
    for source_line in lines {
        if measurer.cancellation_requested() {
            return Vec::new();
        }
        if source_line.is_empty() {
            wrapped.push(Vec::new());
            continue;
        }
        probe.reset();
        for (word_index, word) in source_line.iter().enumerate() {
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            probe.push_source_word(word, word_index > 0);
        }
        if probe.width_px() <= max_width_px {
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            wrapped.push(source_line.clone());
            continue;
        }

        probe.reset();
        let mut current = Vec::new();
        let mut word_index = 0usize;
        while let Some(word) = source_line.get(word_index) {
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            let checkpoint = probe.checkpoint();
            probe.push_source_word(word, !current.is_empty());
            if probe.width_px() <= max_width_px {
                if measurer.cancellation_requested() {
                    return Vec::new();
                }
                current.push(word.clone());
                word_index += 1;
                continue;
            }
            probe.restore(checkpoint);
            if !current.is_empty() {
                wrapped.push(std::mem::take(&mut current));
                probe.reset();
                continue;
            }
            if !break_long_words {
                wrapped.push(vec![word.clone()]);
                word_index += 1;
                continue;
            }

            let chunks = split_source_word_to_fit(&mut probe, word, max_width_px);
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            for chunk in chunks {
                wrapped.push(vec![chunk]);
            }
            probe.reset();
            word_index += 1;
        }
        if !current.is_empty() {
            wrapped.push(current);
        }
    }

    if wrapped.is_empty() {
        vec![Vec::new()]
    } else {
        wrapped
    }
}

// Mermaid 11.16 inserts decoded labels through `addHtmlSpan(...).html(...)`. HTML collapses the
// ASCII whitespace set at a boundary, but U+00A0 remains visible and contributes to the line box.
pub(crate) fn flowchart_trim_html_collapsible_whitespace(input: &str) -> &str {
    trim_html_collapsible_ascii_whitespace(input)
}

pub(crate) fn flowchart_label_text_is_empty_for_mode(text: &str, _html_labels: bool) -> bool {
    flowchart_trim_html_collapsible_whitespace(text).is_empty()
}

pub(crate) fn flowchart_label_is_empty_for_render(label: &str) -> bool {
    // Mermaid admits an edge label whenever FlowDB's already-trimmed label string is non-empty.
    // Do not infer emptiness from extracted text: image-, icon-, and math-only HTML labels still
    // have visible DOM content, and parsing here would duplicate the later measurement work.
    label.is_empty()
}

pub(crate) fn flowchart_label_metrics_for_layout(
    request: FlowchartLabelMetricsRequest<'_>,
) -> crate::text::TextMetrics {
    let resolved_config;
    let request = if request.config.get_str("securityLevel").is_none() {
        // Internal standalone measurement also accepts an empty config. Match the renderer's
        // effective default instead of escaping its generated paragraph as literal text.
        resolved_config = flowchart_label_sanitize_config(request.config);
        FlowchartLabelMetricsRequest {
            config: resolved_config.as_ref(),
            ..request
        }
    } else {
        request
    };
    if request.raw_label.is_empty() {
        return crate::text::TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        };
    }
    if let Some(metrics) =
        crate::math::math_label_metrics_for_layout(crate::math::MathLabelMetricsRequest {
            measurer: request.measurer,
            raw_label: request.raw_label,
            style: request.style,
            max_width_px: request.max_width_px,
            wrap_mode: request.wrap_mode,
            config: request.config,
            math_renderer: request.math_renderer,
        })
    {
        return metrics;
    }
    if request.wrap_mode == WrapMode::HtmlLike {
        let source =
            flowchart_label_html_source(request.raw_label, request.label_type, request.config);
        let xhtml = merman_core::sanitize::sanitize_xhtml(&source, request.config.as_config());
        return flowchart_html_label_metrics_for_layout(request, &xhtml);
    }
    if request.label_type == "markdown" {
        crate::text::measure_wrapped_markdown_with_inline_styles(
            request.measurer,
            request.raw_label,
            request.style,
            request.max_width_px,
            request.wrap_mode,
        )
    } else {
        FlowchartSvgLabelSource::new(request.raw_label).metrics(
            request.measurer,
            request.style,
            request.max_width_px,
            FlowchartSvgWidthMode::Bbox,
        )
    }
}

pub(crate) fn flowchart_label_sanitize_config(
    config: &PresentationConfig,
) -> std::borrow::Cow<'_, PresentationConfig> {
    if config.get_str("securityLevel").is_some() {
        return std::borrow::Cow::Borrowed(config);
    }
    let mut config = config.clone();
    config.set_value("securityLevel", serde_json::Value::String("strict".into()));
    std::borrow::Cow::Owned(config)
}

/// Measures the exact sanitized XHTML retained for the corresponding HTML-label terminal.
pub(crate) fn flowchart_html_label_metrics_for_layout(
    request: FlowchartLabelMetricsRequest<'_>,
    xhtml: &str,
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
    if let Some(plain) = crate::text::mermaid_xhtml_label_plain_text(xhtml)
        && !plain.contains('<')
    {
        // A host profile may provide the complete paragraph's browser measurement. Keep that
        // request intact after collapsing rendered whitespace, instead of probing each word.
        let plain = flowchart_label_plain_text_for_layout(xhtml, "html", true);
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
                        merman_core::entities::decode_html_entities_to_unicode(text.as_ref());
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
                    let decoded = merman_core::entities::decode_html_entities_to_unicode(&encoded);
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
        measure_flowchart_html_images(measurer, xhtml, style, max_width_px, fixed_img_width)
    } else if crate::text::flowchart_html_has_inline_style_tags(&lower)
        || lower.contains("<i ")
        || lower.contains("<br")
        || (label_type == "markdown" && !markdown_has_unwrapped_text)
    {
        crate::text::measure_html_with_inline_styles(
            measurer,
            xhtml,
            style,
            max_width_px,
            WrapMode::HtmlLike,
        )
    } else {
        let plain = flowchart_label_plain_text_for_layout(xhtml, "html", true);
        if plain.is_empty() {
            crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            }
        } else if plain.contains('<') {
            crate::text::measure_html_with_inline_styles(
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
    measurer: &dyn TextMeasurer,
    html: &str,
    style: &TextStyle,
    max_width_px: Option<f64>,
    fixed_img_width: bool,
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
            let Some(rel_end) = rest.find('>') else {
                // No later tag can close either; preserve the remaining text once.
                text_buf.push_str(rest);
                break;
            };
            if rest
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<img"))
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
            if rest
                .get(..3)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<br"))
            {
                text_buf.push('\n');
                i += rel_end + 1;
                continue;
            }
            i += rel_end + 1;
            continue;
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
        // The scanner separates already-sanitized markup before measuring its DOM text. Decode
        // the XHTML representation once so entities do not become visible glyph spellings.
        let input = merman_core::entities::decode_html_entities_to_unicode(input);
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
                let m = measurer.measure_wrapped(&t, style, Some(max_width), WrapMode::HtmlLike);
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

pub(crate) fn flowchart_non_markdown_label_for_html(label: &str, label_type: &str) -> String {
    let mut label = if label.contains("\r\n") {
        label.replace("\r\n", "\n")
    } else {
        label.to_string()
    };
    label = flowchart_decode_label_escapes(&label);
    if label_type == "string" {
        label = flowchart_trim_html_collapsible_whitespace(&label).to_string();
    }
    let label = label.trim_end_matches('\n');
    if !label.contains("\\n") && !label.contains('\n') {
        return label.to_string();
    }

    let mut out = String::with_capacity(label.len());
    let mut offset = 0usize;
    while offset < label.len() {
        let rest = &label[offset..];
        if rest.starts_with("\\n") {
            out.push_str("<br />");
            offset += 2;
            continue;
        }
        let ch = rest
            .chars()
            .next()
            .expect("offset before label end has a character");
        if ch == '\n' {
            out.push_str("<br />");
        } else {
            out.push(ch);
        }
        offset += ch.len_utf8();
    }
    out
}

pub(crate) fn flowchart_label_plain_text_for_layout(
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
                let decoded = merman_core::entities::decode_html_entities_to_unicode(run.as_str());
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
                return crate::text::mermaid_markdown_to_lines(label, true)
                    .into_iter()
                    .map(|line| {
                        line.into_iter()
                            .map(
                                |(word, _)| match crate::entities::decode_svg_text_content_entities(
                                    &word,
                                ) {
                                    std::borrow::Cow::Borrowed(_) => word,
                                    std::borrow::Cow::Owned(decoded) => decoded,
                                },
                            )
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
            }

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
            flowchart_trim_html_collapsible_whitespace(&out).to_string()
        }
        _ => {
            if !html_labels {
                let word_lines = flowchart_non_markdown_svg_source_word_lines(label);
                return flowchart_svg_source_word_lines_plain_text(&word_lines);
            }

            let mut t = flowchart_decode_label_escapes(&label.replace("\r\n", "\n"));
            // Keep the raw label text for layout, then strip HTML tags/entities.
            //
            // Note: in Mermaid flowchart-v2, FontAwesome icon tokens (e.g. `fa:fa-car`)
            // can affect the measured label width even though the exported SVG replaces them
            // with empty `<i class="fa ..."></i>` nodes (FontAwesome CSS is not embedded).
            // For strict parity we therefore *do not* rewrite the `fa:` token here.
            t = strip_html_for_layout(&t);
            t.trim_matches(|ch| matches!(ch, '\t' | '\u{000C}' | '\r' | ' '))
                .to_string()
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
pub(crate) fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    match compute_bounds_impl(nodes, edges, |_| Ok::<(), std::convert::Infallible>(())) {
        Ok(bounds) => bounds,
        Err(never) => match never {},
    }
}

pub(crate) fn compute_bounds_controlled(
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    charge: impl FnMut(usize) -> crate::Result<()>,
) -> crate::Result<Option<Bounds>> {
    compute_bounds_impl(nodes, edges, charge)
}

#[inline]
pub(crate) fn starts_with_ascii_case_insensitive(input: &str, prefix: &[u8]) -> bool {
    input
        .as_bytes()
        .get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

#[inline]
fn find_ascii_case_insensitive(input: &str, needle: &[u8]) -> Option<usize> {
    debug_assert!(!needle.is_empty());
    input
        .as_bytes()
        .windows(needle.len())
        .position(|candidate| candidate.eq_ignore_ascii_case(needle))
}

fn extract_flowchart_img_src(tag: &str) -> Option<&str> {
    let idx = find_ascii_case_insensitive(tag, b"src=")?;
    let rest = tag.get(idx + 4..)?.trim_start();
    let quote = *rest.as_bytes().first()?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }

    let value = rest.get(1..)?;
    let end = value
        .as_bytes()
        .iter()
        .position(|candidate| *candidate == quote)
        .unwrap_or(value.len());
    let value = value.get(..end)?.trim();
    (!value.is_empty()).then_some(value)
}

pub(crate) fn normalize_flowchart_img_tags(input: &str, fixed_width: bool) -> String {
    // Mermaid flowchart-v2 adds inline styles to `<img>` tags inside HTML labels to constrain
    // their layout. The SVG baseline uses XHTML, so we also self-close the tags later.
    if find_ascii_case_insensitive(input, b"<img").is_none() {
        return input.to_string();
    }

    let style = if fixed_width {
        "display: flex; flex-direction: column; min-width: 80px; max-width: 80px;"
    } else {
        "display: flex; flex-direction: column; width: 100%;"
    };

    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'<' && starts_with_ascii_case_insensitive(&input[i..], b"<img") {
            let rest = &input[i..];
            let Some(rel_end) = rest.find('>') else {
                out.push_str(rest);
                break;
            };
            let tag = &rest[..=rel_end];
            let src = extract_flowchart_img_src(tag);
            out.push_str("<img");
            if let Some(src) = src {
                let _ = write!(out, r#" src="{}""#, crate::svg::escape_attr(src));
            }
            out.push_str(r#" style=""#);
            out.push_str(style);
            out.push('"');
            out.push('>');
            i += rel_end + 1;
            continue;
        }
        let Some(ch) = input[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

pub(crate) fn is_single_img_label(label: &str) -> bool {
    let label = flowchart_trim_html_collapsible_whitespace(label);
    if !starts_with_ascii_case_insensitive(label, b"<img") {
        return false;
    }
    let Some(end) = label.find('>') else {
        return false;
    };
    flowchart_trim_html_collapsible_whitespace(&label[end + 1..]).is_empty()
}

pub(crate) fn flowchart_label_html_source(
    label: &str,
    label_type: &str,
    config: &merman_core::presentation::PresentationConfig,
) -> String {
    if flowchart_label_text_is_empty_for_mode(label, true) {
        return String::new();
    }

    fn trim_markdown_trailing_newlines(
        input: std::borrow::Cow<'_, str>,
    ) -> std::borrow::Cow<'_, str> {
        if input.ends_with('\n') {
            std::borrow::Cow::Owned(input.trim_end_matches('\n').to_string())
        } else {
            input
        }
    }

    match label_type {
        "markdown" => {
            let decoded = crate::entities::decode_mermaid_entities_for_render_text(label);
            let decoded = if decoded.contains("\\\\") {
                std::borrow::Cow::Owned(decoded.replace("\\\\", "\\"))
            } else {
                decoded
            };
            let decoded = trim_markdown_trailing_newlines(decoded);
            let markdown_auto_wrap = config
                .as_value()
                .get("markdownAutoWrap")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let html_out = crate::text::mermaid_markdown_to_html_label_fragment(
                decoded.as_ref(),
                markdown_auto_wrap,
            );
            let html_out = flowchart_trim_html_collapsible_whitespace(&html_out).to_string();
            crate::text::replace_fontawesome_icons(&html_out)
        }
        _ => {
            let label = flowchart_non_markdown_label_for_html(label, label_type);

            // Mermaid's nonMarkdownToHTML() wraps every non-empty label in one paragraph.
            // Markdown block classification must not leak into this branch.
            let fixed_img_width = is_single_img_label(&label);
            let label = normalize_flowchart_img_tags(&label, fixed_img_width);
            let wrapped = format!("<p>{}</p>", label);
            if wrapped.contains(":fa-") {
                crate::text::replace_fontawesome_icons(&wrapped)
            } else {
                wrapped
            }
        }
    }
}

#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
mod tests {
    use super::*;
    use crate::math::MathRenderer;
    use crate::model::{LayoutLabel, LayoutPoint};

    #[test]
    fn authored_entity_scan_is_bounded_to_sixty_four_bytes() {
        let longest_admitted = format!("&{};", "a".repeat(62));
        let first_rejected = format!("&{};", "a".repeat(63));
        let amplified = format!("{};", "&".repeat(16_384));

        assert_eq!(authored_entity_len(&longest_admitted), Some(64));
        assert_eq!(authored_entity_len(&first_rejected), None);
        assert_eq!(authored_entity_len(&amplified), None);
        assert_eq!(
            FlowchartSvgLabelSource::new(&amplified).plain_text(),
            amplified
        );
    }

    #[test]
    fn authored_entity_scan_preserves_unicode_entity_semantics() {
        assert_eq!(authored_entity_len("&é;"), Some(4));
        assert_eq!(authored_entity_len("&\u{2003};"), None);

        let split_at_scan_boundary = format!("&{};", "é".repeat(32));
        assert_eq!(authored_entity_len(&split_at_scan_boundary), None);
        assert_eq!(
            FlowchartSvgLabelSource::new(&split_at_scan_boundary).plain_text(),
            split_at_scan_boundary
        );
    }

    #[test]
    fn source_plain_projection_preserves_unknown_entities_and_source_breaks() {
        let source = FlowchartSvgLabelSource::new("alpha &amp; &bad&amp; &unknown;\nbeta");
        assert_eq!(source.plain_text(), "alpha & &bad&amp; &unknown;\nbeta");
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    #[test]
    fn node_minimum_excludes_circles_fixed_symbols_assets_and_empty_labels() {
        let config = PresentationConfig::from_value(serde_json::json!({
            "flowchart": { "minNodeWidth": 120.0 }
        }));
        assert_eq!(
            flowchart_node_label_min_width("X", Some("rect"), &config),
            120.0
        );
        assert_eq!(
            flowchart_node_label_min_width("", Some("rect"), &config),
            0.0
        );
        for shape in [
            "circle",
            "circ",
            "doublecircle",
            "dbl-circ",
            "double-circle",
            "icon",
            "iconCircle",
            "iconRounded",
            "iconSquare",
            "imageSquare",
            "anchor",
            "choice",
            "cross-circ",
            "f-circ",
            "fork",
            "stop",
            "start",
            "bolt",
            "collapsedGroup",
        ] {
            assert_eq!(
                flowchart_node_label_min_width("X", Some(shape), &config),
                0.0,
                "{shape}"
            );
        }
    }

    #[derive(Debug)]
    struct PreciseMathRenderer;

    impl MathRenderer for PreciseMathRenderer {
        fn render_html_label(&self, text: &str, _config: &PresentationConfig) -> Option<String> {
            text.contains("$$").then(|| text.to_string())
        }

        fn measure_html_label(
            &self,
            text: &str,
            _config: &PresentationConfig,
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

    #[derive(Default)]
    struct ImageTextMeasurer(std::cell::RefCell<Vec<String>>);

    impl TextMeasurer for ImageTextMeasurer {
        fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
            self.0.borrow_mut().push(text.to_string());
            TextMetrics {
                width: 1.001,
                height: 2.002,
                line_count: 1,
            }
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            max_width: Option<f64>,
            wrap_mode: WrapMode,
        ) -> TextMetrics {
            assert_eq!(max_width, Some(120.0));
            assert_eq!(wrap_mode, WrapMode::HtmlLike);
            self.measure(text, style)
        }
    }

    fn image_label_metrics(label: &str, measurer: &dyn TextMeasurer) -> TextMetrics {
        flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer,
            raw_label: label,
            label_type: "html",
            style: &TextStyle::default(),
            max_width_px: Some(120.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &PresentationConfig::default(),
            math_renderer: None,
        })
    }

    #[test]
    fn html_image_layout_preserves_metrics_and_measurement_requests() {
        // Prefix and src recognition are deliberately the existing headless rules.
        let cases: &[(&str, &[&str], usize, f64)] = &[
            ("<img src='x'>", &[], 1, 80.0),
            ("<IMG src=''>", &[], 0, 80.0),
            (
                "<span>中😀</span><IMGx src='图'><BRx>尾",
                &["中😀", "尾"],
                1,
                120.0,
            ),
            ("a<im😀>b<img src='x'>", &["ab"], 1, 120.0),
            ("a<img😀 src='x'><br中>b", &["a", "b"], 1, 120.0),
            ("a<img>", &["a"], 0, 120.0),
            ("a<img src='  '>", &["a"], 0, 120.0),
            ("a<img src=x>", &["a"], 0, 120.0),
            ("a<img src = 'x'>", &["a"], 0, 120.0),
            (
                "<span style='min-width: 80px;'>long text</span><img src='x'>",
                &["long text"],
                1,
                120.0,
            ),
            ("a<img data-src='x'>", &["a"], 1, 120.0),
            ("a<img src='a>b'>尾", &["a", "b'>尾"], 1, 120.0),
            (
                "<span> \tA\u{00a0} </span><BR><br /> B <img SRC=' x '> C",
                &["A\u{00a0}\n\nB", "C"],
                1,
                120.0,
            ),
            (
                "a<img src='x'>b<img src=''>c<img src='y'>d",
                &["a", "b", "c", "d"],
                2,
                120.0,
            ),
            // HTML5 recovery preserves these visible stray angle brackets as text.
            ("head<img src='x'>tail<<中", &["head", "tail<<中"], 1, 120.0),
        ];
        for &(label, expected_requests, images_with_src, width) in cases {
            let measurer = ImageTextMeasurer::default();
            let metrics = image_label_metrics(label, &measurer);
            assert_eq!(*measurer.0.borrow(), expected_requests, "{label}");
            assert_eq!(metrics.width, width, "{label}");
            let height = images_with_src as f64 * width + expected_requests.len() as f64 * 2.002;
            assert_eq!(
                metrics.height,
                crate::text::ceil_to_1_64_px(height),
                "{label}"
            );
            assert_eq!(
                metrics.line_count,
                images_with_src + expected_requests.len(),
                "{label}"
            );
        }
    }

    #[test]
    fn incomplete_image_tag_uses_final_plain_text_measurement() {
        let config =
            PresentationConfig::from_value(serde_json::json!({ "securityLevel": "strict" }));
        let html = flowchart_label_html_source("head<img", "html", &config);
        let xhtml = merman_core::sanitize::sanitize_xhtml(&html, config.as_config());
        let document = roxmltree::Document::parse(&xhtml)
            .expect("HTML5 recovery closes the paragraph after discarding the incomplete image");
        assert!(!document.descendants().any(|node| node.has_tag_name("img")));
        assert_eq!(document.root_element().text(), Some("head"));

        let measurer = ImageTextMeasurer::default();
        let metrics = image_label_metrics("head<img", &measurer);
        assert_eq!(*measurer.0.borrow(), ["head"]);
        assert_eq!(metrics.width, 1.001);
        assert_eq!(metrics.height, 2.002);
        assert_eq!(metrics.line_count, 1);
    }

    #[test]
    fn html_image_layout_many_tags_preserves_text_and_metrics() {
        for count in [1, 32, 512, 4096] {
            let plain = format!("{}<img src='x'>", "α".repeat(count));
            let tagged = format!("{}<iMg src='x'>", "<span>α</span>".repeat(count));
            let plain_measurer = ImageTextMeasurer::default();
            let tagged_measurer = ImageTextMeasurer::default();
            let plain_metrics = image_label_metrics(&plain, &plain_measurer);
            let tagged_metrics = image_label_metrics(&tagged, &tagged_measurer);
            assert_eq!(tagged_metrics.width, plain_metrics.width, "count={count}");
            assert_eq!(tagged_metrics.height, plain_metrics.height, "count={count}");
            assert_eq!(
                tagged_metrics.line_count, plain_metrics.line_count,
                "count={count}"
            );
            assert_eq!(
                *tagged_measurer.0.borrow(),
                *plain_measurer.0.borrow(),
                "count={count}"
            );
        }
    }

    #[test]
    fn html_plain_text_discards_only_synthetic_terminal_block_breaks() {
        assert_eq!(
            flowchart_label_plain_text_for_layout("<p>approval</p>", "html", true),
            "approval"
        );
        assert_eq!(
            flowchart_label_plain_text_for_layout("<p>&nbsp;Edge&nbsp;</p>", "html", true,),
            "\u{00A0}Edge\u{00A0}"
        );
        assert_eq!(
            flowchart_label_plain_text_for_layout("<p>A<br>&nbsp;</p>", "html", true),
            "A\n\u{00A0}"
        );
        assert_eq!(
            flowchart_label_plain_text_for_layout("<p>A<br></p>", "html", true),
            "A\n"
        );
    }

    #[test]
    fn mixed_math_metrics_preserve_fragment_precision() {
        let config = PresentationConfig::default();
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
        let config = PresentationConfig::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();

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
            assert!(
                metrics.height.is_finite() && metrics.height > style.font_size,
                "{raw_label:?}: {metrics:?}"
            );
        }
    }

    #[test]
    fn html_label_metrics_collapse_plain_paragraph_spaces_but_preserve_nbsp() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = PresentationConfig::default();
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "A&nbsp;&nbsp;B  C   D",
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected = measurer.measure_wrapped(
            "A\u{00a0}\u{00a0}B C D",
            &style,
            Some(200.0),
            WrapMode::HtmlLike,
        );
        assert_eq!(metrics.width, expected.width);
        assert_eq!(metrics.height, expected.height);
        assert_eq!(metrics.line_count, expected.line_count);
    }

    #[test]
    fn markdown_html_label_metrics_keep_the_browser_width_lattice() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = PresentationConfig::default();
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "Label",
            label_type: "markdown",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected =
            crate::text::round_to_1_64_px(measurer.measure_html_line_width_px("Label", &style));
        assert_eq!(metrics.width, expected);
    }

    #[test]
    fn markdown_html_raw_block_text_keeps_plain_measurement_precision() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = PresentationConfig::default();
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "Hello\n  - l1\n  - l2",
            label_type: "markdown",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected =
            measurer.measure_wrapped("Hello\n- l1 - l2", &style, Some(200.0), WrapMode::HtmlLike);
        assert_eq!(metrics.width, expected.width);
        assert_eq!(metrics.height, expected.height);
        assert_eq!(metrics.line_count, expected.line_count);
    }

    #[test]
    fn markdown_html_min_content_overflow_preserves_host_precision() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_family: Some("\"Recursive Variable\",arial,sans-serif".to_string()),
            font_size: 14.0,
            ..TextStyle::default()
        };
        let config = PresentationConfig::from_value(serde_json::json!({
            "markdownAutoWrap": true,
            "securityLevel": "strict"
        }));
        let raw_label = "**angular_velocity**\n      *angular_displacement / duration*\n      [rad/s, 1/s]\n      {vector}";
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label,
            label_type: "markdown",
            style: &style,
            max_width_px: Some(120.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected = measurer.measure_wrapped(
            "angular_velocity\nangular_displacement / duration\n[rad/s, 1/s]\n{vector}",
            &style,
            Some(120.0),
            WrapMode::HtmlLike,
        );
        assert!(expected.width > 120.0);
        assert_eq!(metrics.width, expected.width);
        assert_eq!(metrics.height, expected.height);
        assert_eq!(metrics.line_count, expected.line_count);
    }

    #[test]
    fn markdown_html_min_content_overflow_preserves_authored_html_styles() {
        let measurer =
            crate::text::DeterministicTextMeasurer::default().with_width_callback(|text, style| {
                let advance = if style.font_weight.as_deref() == Some("700") {
                    20.0
                } else {
                    10.0
                };
                text.chars().count() as f64 * advance
            });
        let style = TextStyle::default();
        let config = PresentationConfig::from_value(serde_json::json!({
            "securityLevel": "strict"
        }));
        let word = "abcdefghijklmnopqrstuvwx";
        for (raw_label, expected_width) in [
            (word.to_string(), 240.0),
            (format!("**{word}**"), 240.0),
            (format!("<strong>{word}</strong>"), 480.0),
            (format!("<div><strong>{word}</strong></div>"), 480.0),
        ] {
            for max_width in [40.0, 120.0] {
                let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                    measurer: measurer.as_ref(),
                    raw_label: &raw_label,
                    label_type: "markdown",
                    style: &style,
                    max_width_px: Some(max_width),
                    wrap_mode: WrapMode::HtmlLike,
                    config: &config,
                    math_renderer: None,
                });
                assert_eq!(metrics.width, expected_width, "{raw_label}, {max_width}");
                assert_eq!(metrics.line_count, 1, "{raw_label}, {max_width}");
            }
        }
    }

    #[test]
    fn markdown_html_entity_only_root_text_keeps_plain_measurement_precision() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = PresentationConfig::from_value(serde_json::json!({
            "securityLevel": "strict",
            "dompurifyConfig": { "FORBID_TAGS": ["p"] }
        }));
        let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "&amp;",
            label_type: "markdown",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: None,
        });
        let expected = measurer.measure_wrapped("&", &style, Some(200.0), WrapMode::HtmlLike);
        assert_eq!(metrics.width, expected.width);
        assert_eq!(metrics.line_count, expected.line_count);
    }

    #[test]
    fn markdown_html_entity_text_prevents_an_image_only_extra_line() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        let config = PresentationConfig::default();
        let measure = |xhtml| {
            flowchart_html_label_metrics_for_layout(
                FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: "",
                    label_type: "markdown",
                    style: &style,
                    max_width_px: Some(200.0),
                    wrap_mode: WrapMode::HtmlLike,
                    config: &config,
                    math_renderer: None,
                },
                xhtml,
            )
        };
        let entity = measure("<p>&amp;<img src=\"x\" /></p>");
        let plain = measure("<p>X<img src=\"x\" /></p>");
        assert_eq!(entity.line_count, plain.line_count);
        assert_eq!(entity.height, plain.height);
    }

    #[test]
    fn non_markdown_html_metrics_share_create_text_explicit_line_breaks() {
        let config = PresentationConfig::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();

        for (raw_label, expected_lines) in [
            ("第一行\n第二行", 2),
            ("first\\nsecond\\nthird", 3),
            ("A<br><br>B", 3),
            ("<br>", 1),
            ("<br><br>", 2),
        ] {
            let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type: "text",
                style: &style,
                max_width_px: Some(200.0),
                wrap_mode: WrapMode::HtmlLike,
                config: &config,
                math_renderer: None,
            });
            assert_eq!(
                metrics.line_count, expected_lines,
                "{raw_label:?}: {metrics:?}"
            );
            assert_eq!(
                metrics.height,
                expected_lines as f64 * 24.0,
                "{raw_label:?}: {metrics:?}"
            );
        }
    }

    #[test]
    fn svg_non_markdown_closing_br_uses_the_sanitized_break_semantics() {
        let text = flowchart_label_plain_text_for_layout(
            "`This is **bold** </br>and <strong>strong</strong>`",
            "text",
            false,
        );

        assert_eq!(text, "`This is **bold**\nand <strong> strong </strong> `");
        assert!(!text.contains("</br>"));
    }

    #[test]
    fn svg_non_markdown_words_preserve_source_tag_provenance() {
        let raw = flowchart_non_markdown_svg_source_word_lines("<span class='foo bar'>X</span>");
        assert_eq!(
            raw,
            vec![vec![
                "<span class='foo bar'>".to_string(),
                "X".to_string(),
                "</span>".to_string(),
            ]],
        );
        assert_eq!(
            flowchart_svg_source_word_lines_plain_text(&raw),
            "<span class='foo bar'> X </span>"
        );

        let encoded = flowchart_non_markdown_svg_source_word_lines(
            "&lt;span class='foo bar'&gt;X&lt;/span&gt;",
        );
        assert_eq!(
            encoded,
            vec![vec![
                "&lt;span".to_string(),
                "class='foo".to_string(),
                "bar'&gt;X&lt;/span&gt;".to_string(),
            ]],
        );
        assert_eq!(
            flowchart_svg_source_word_lines_plain_text(&encoded),
            "<span class='foo bar'>X</span>"
        );

        let decoded_angle = flowchart_non_markdown_svg_source_word_lines("&lt;Less&lt;");
        assert_eq!(decoded_angle, vec![vec!["&lt;Less&lt;".to_string()]]);
        assert_eq!(
            flowchart_svg_source_word_lines_plain_text(&decoded_angle),
            "<Less<"
        );

        let short_non_ascii = flowchart_non_markdown_svg_source_word_lines("<é");
        assert_eq!(short_non_ascii, vec![vec!["é".to_string()]]);
    }

    #[test]
    fn svg_long_word_wrapping_preserves_grapheme_clusters() {
        struct ScalarWidthMeasurer;

        impl TextMeasurer for ScalarWidthMeasurer {
            fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: text.chars().count() as f64 * 10.0,
                    height: 10.0,
                    line_count: 1,
                }
            }
        }

        let source = vec![vec!["e\u{0301}x".to_string(), "👨‍👩‍👧‍👦y".to_string()]];
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &ScalarWidthMeasurer,
            &source,
            &TextStyle::default(),
            Some(5.0),
            true,
        );

        assert_eq!(
            wrapped,
            vec![
                vec!["e\u{0301}".to_string()],
                vec!["x".to_string()],
                vec!["👨‍👩‍👧‍👦".to_string()],
                vec!["y".to_string()],
            ]
        );
    }

    #[test]
    fn svg_word_wrapping_uses_computed_text_length_instead_of_bbox_width() {
        struct DivergentSvgWidthMeasurer;

        impl TextMeasurer for DivergentSvgWidthMeasurer {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: 100.0,
                    height: 10.0,
                    line_count: 1,
                }
            }

            fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
                text.chars().count() as f64 * 10.0
            }
        }

        let source = vec![vec!["ab".to_string(), "cd".to_string()]];
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &DivergentSvgWidthMeasurer,
            &source,
            &TextStyle::default(),
            Some(50.0),
            true,
        );

        assert_eq!(wrapped, source);
    }

    #[test]
    fn svg_word_wrapping_preserves_opaque_measurer_request_order() {
        struct RecordingMeasurer {
            calls: std::cell::RefCell<Vec<String>>,
        }

        impl TextMeasurer for RecordingMeasurer {
            fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: text.chars().count() as f64 * 10.0,
                    height: 10.0,
                    line_count: 1,
                }
            }

            fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
                self.calls.borrow_mut().push(text.to_string());
                text.chars().count() as f64 * 10.0
            }
        }

        let measurer = RecordingMeasurer {
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let source = vec![vec!["aa".to_string(), "bb".to_string(), "cc".to_string()]];
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &measurer,
            &source,
            &TextStyle::default(),
            Some(25.0),
            true,
        );

        assert_eq!(
            wrapped,
            vec![
                vec!["aa".to_string()],
                vec!["bb".to_string()],
                vec!["cc".to_string()],
            ]
        );
        assert_eq!(
            measurer.calls.into_inner(),
            ["aa bb cc", "aa", "aa bb", "bb", "bb cc", "cc"]
        );

        let measurer = RecordingMeasurer {
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &measurer,
            &[vec!["abcd".to_string()]],
            &TextStyle::default(),
            Some(15.0),
            true,
        );
        assert_eq!(
            wrapped,
            ["a", "b", "c", "d"]
                .into_iter()
                .map(|word| vec![word.to_string()])
                .collect::<Vec<_>>()
        );
        assert_eq!(
            measurer.calls.into_inner(),
            [
                "abcd", "abcd", "a", "ab", "bcd", "b", "bc", "cd", "c", "cd", "d"
            ]
        );
    }

    #[test]
    fn builtin_svg_word_wrapping_streams_long_grapheme_labels() {
        struct StreamingOnlyMeasurer;

        impl TextMeasurer for StreamingOnlyMeasurer {
            #[allow(private_interfaces)]
            fn begin_svg_text_computed_length(
                &self,
                style: &TextStyle,
            ) -> Option<crate::environment::BuiltinSvgComputedLength> {
                Some(crate::environment::BuiltinSvgComputedLength::deterministic(
                    style,
                ))
            }

            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: 0.0,
                    height: 10.0,
                    line_count: 1,
                }
            }

            fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
                panic!("the built-in streaming path must not rescan complete prefixes")
            }
        }

        let word = "a".repeat(4_096);
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &StreamingOnlyMeasurer,
            &[vec![word]],
            &TextStyle::default(),
            Some(1.0),
            true,
        );

        assert_eq!(wrapped.len(), 4_096);
        assert!(wrapped.iter().all(|line| line == &["a".to_string()]));
    }

    #[test]
    fn svg_word_wrapping_stops_when_the_measurement_operation_is_cancelled() {
        struct CancellingMeasurer {
            calls: std::cell::Cell<usize>,
            cancelled: std::cell::Cell<bool>,
        }

        impl TextMeasurer for CancellingMeasurer {
            fn cancellation_requested(&self) -> bool {
                self.cancelled.get()
            }

            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: 1.0,
                    height: 10.0,
                    line_count: 1,
                }
            }

            fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
                let calls = self.calls.get().saturating_add(1);
                self.calls.set(calls);
                self.cancelled.set(true);
                text.chars().count() as f64
            }
        }

        let measurer = CancellingMeasurer {
            calls: std::cell::Cell::new(0),
            cancelled: std::cell::Cell::new(false),
        };
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &measurer,
            &[vec!["a".repeat(100_000)]],
            &TextStyle::default(),
            Some(1.0),
            true,
        );

        assert!(wrapped.is_empty());
        assert_eq!(measurer.calls.get(), 1);
    }

    #[test]
    fn non_markdown_html_preserves_internal_line_boundary_whitespace() {
        assert_eq!(
            flowchart_non_markdown_label_for_html("A\n  B  \nC", "text"),
            "A<br />  B  <br />C"
        );
        assert_eq!(
            flowchart_non_markdown_label_for_html("  A\n  B  \nC  ", "string"),
            "A<br />  B  <br />C"
        );
    }

    #[test]
    fn empty_flowchart_labels_have_zero_layout_metrics_in_both_render_modes() {
        let config = PresentationConfig::default();
        let style = TextStyle::default();
        let measurer = PreciseTextMeasurer;

        for wrap_mode in [WrapMode::HtmlLike, WrapMode::SvgLike] {
            for label_type in ["text", "string", "markdown"] {
                let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                    measurer: &measurer,
                    raw_label: "",
                    label_type,
                    style: &style,
                    max_width_px: None,
                    wrap_mode,
                    config: &config,
                    math_renderer: None,
                });

                assert_eq!(metrics.width, 0.0, "{wrap_mode:?} {label_type}");
                assert_eq!(metrics.height, 0.0, "{wrap_mode:?} {label_type}");
                assert_eq!(metrics.line_count, 0, "{wrap_mode:?} {label_type}");
            }
        }
    }

    #[test]
    fn svg_label_metrics_apply_create_text_nbsp_rules_by_label_type() {
        let config = PresentationConfig::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let measure = |raw_label: &str, label_type: &str| {
            flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label,
                label_type,
                style: &style,
                max_width_px: Some(200.0),
                wrap_mode: WrapMode::SvgLike,
                config: &config,
                math_renderer: None,
            })
        };

        let text = measure("\u{00A0}", "text");
        assert_eq!(text.width, 0.0, "{text:?}");
        assert_eq!(text.height, 0.0, "{text:?}");
        assert_eq!(text.line_count, 0, "{text:?}");

        let markdown = measure("\u{00A0}", "markdown");
        assert!(markdown.width > 0.0, "{markdown:?}");
        assert!(markdown.height > 0.0, "{markdown:?}");
        assert_eq!(markdown.line_count, 1, "{markdown:?}");

        let entity = measure("&nbsp;", "text");
        assert!(entity.width > markdown.width, "{entity:?} {markdown:?}");
        assert!(entity.height > 0.0, "{entity:?}");
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
