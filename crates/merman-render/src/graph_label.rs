use crate::html::label_box::{HtmlLabelBox, HtmlLabelWidth};
#[cfg(feature = "markdown-labels")]
use crate::math::MathRenderer;
use crate::mermaid_style::HtmlTextGeometryDeclarations;
#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
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
    TextMeasurer, TextStyle, WrapMode, is_html_collapsible_ascii_whitespace,
    trim_html_collapsible_ascii_whitespace,
};
use merman_model::presentation::PresentationConfig;
#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-mindmap"
    )
))]
pub(crate) use merman_render_core::text::flowchart_wrap_svg_source_word_lines;
use merman_render_core::text::literal_label::flowchart_decode_label_escapes;
pub(crate) use merman_render_core::text::literal_label::{
    FlowchartSvgLabelSource, FlowchartSvgWidthMode, flowchart_non_markdown_label_for_html,
    flowchart_non_markdown_svg_plain_text, flowchart_non_markdown_svg_source_word_lines,
};
use quick_xml::{
    Reader, XmlVersion,
    events::{BytesStart, Event},
};
use std::borrow::Cow;
use std::fmt::Write as _;

#[cfg(feature = "markdown-labels")]
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
            Some(
                FlowchartShape::Anchor
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
                    | FlowchartShape::CollapsedGroup
            )
        )
    {
        return 0.0;
    }
    crate::config::config_f64(config, &["flowchart", "minNodeWidth"])
        .unwrap_or(0.0)
        .max(0.0)
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

#[cfg(feature = "markdown-labels")]
pub(crate) fn flowchart_label_metrics_for_layout(
    request: FlowchartLabelMetricsRequest<'_>,
) -> crate::text::TextMetrics {
    flowchart_label_metrics_with_html_geometry(request, HtmlTextGeometryDeclarations::default())
}

#[cfg(feature = "markdown-labels")]
pub(crate) fn flowchart_label_metrics_with_html_geometry(
    request: FlowchartLabelMetricsRequest<'_>,
    geometry: HtmlTextGeometryDeclarations,
) -> crate::text::TextMetrics {
    if request.raw_label.is_empty() {
        return crate::text::TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        };
    }
    if let Some(metrics) = flowchart_math_label_metrics_for_layout(&request) {
        return metrics;
    }
    if request.wrap_mode == WrapMode::HtmlLike {
        let source =
            flowchart_label_html_source(request.raw_label, request.label_type, request.config);
        let sanitize_config = flowchart_label_sanitize_config(request.config);
        let xhtml = merman_markup::sanitize::sanitize_xhtml(&source, &sanitize_config);
        return flowchart_html_label_metrics_with_geometry(request, &xhtml, geometry);
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

#[cfg(feature = "markdown-labels")]
fn flowchart_math_label_metrics_for_layout(
    request: &FlowchartLabelMetricsRequest<'_>,
) -> Option<crate::text::TextMetrics> {
    if request.wrap_mode != WrapMode::HtmlLike || !request.raw_label.contains("$$") {
        return None;
    }
    let math_renderer = request.math_renderer?;
    let resolved_config;
    let config = if request.config.get_str("securityLevel").is_none() {
        let mut values = request.config.as_config().clone();
        values.set_value("securityLevel", serde_json::json!("strict"));
        let parts = merman_model::presentation::FrozenPresentationConfigParts::new(values)
            .with_explicit_config_paths(std::sync::Arc::clone(
                request.config.shared_explicit_config_paths(),
            ));
        // Flowchart's standalone measurement defaults to strict without declaring an assignment.
        // Construction validates the derived values without charging the request's model limits.
        resolved_config = PresentationConfig::try_from_frozen_parts_controlled(
            parts,
            &merman_model::resources::InputResourcePolicy::for_profile(
                merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
            ),
            &merman_model::OperationControl::new(),
        )
        .ok()?
        .ok()?;
        &resolved_config
    } else {
        request.config
    };
    crate::math::math_label_metrics_for_layout(crate::math::MathLabelMetricsRequest {
        measurer: request.measurer,
        raw_label: request.raw_label,
        style: request.style,
        max_width_px: request.max_width_px,
        wrap_mode: request.wrap_mode,
        config,
        math_renderer: Some(math_renderer),
    })
}

pub(crate) fn flowchart_label_sanitize_config(
    config: &PresentationConfig,
) -> std::borrow::Cow<'_, merman_model::MermaidConfig> {
    if config.get_str("securityLevel").is_some() {
        return std::borrow::Cow::Borrowed(config.as_config());
    }
    let mut config = config.as_config().clone();
    config.set_value("securityLevel", serde_json::Value::String("strict".into()));
    std::borrow::Cow::Owned(config)
}

/// Measures the exact sanitized XHTML retained for the corresponding HTML-label terminal.
#[cfg(all(test, feature = "markdown-labels"))]
pub(crate) fn flowchart_html_label_metrics_for_layout(
    request: FlowchartLabelMetricsRequest<'_>,
    xhtml: &str,
) -> crate::text::TextMetrics {
    flowchart_html_label_metrics_with_geometry(
        request,
        xhtml,
        HtmlTextGeometryDeclarations::default(),
    )
}

#[cfg(feature = "markdown-labels")]
pub(crate) fn flowchart_html_label_metrics_with_geometry(
    request: FlowchartLabelMetricsRequest<'_>,
    xhtml: &str,
    geometry: HtmlTextGeometryDeclarations,
) -> crate::text::TextMetrics {
    let dimensions = flowchart_html_label_box(request.measurer, xhtml);
    let width = dimensions.and_then(|dimensions| dimensions.width_in_label(request.max_width_px));
    let mut metrics = if let Some(metrics) = crate::svg::measure_styled_html_label(
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
        crate::text::measure_html_with_authored_width(
            request.measurer,
            xhtml,
            request.style,
            width.wrap,
        )
    } else {
        flowchart_html_label_content_metrics(request, xhtml, geometry, width)
    };
    if let Some(width) = width {
        metrics.width = width.outer;
    }
    if let Some(dimensions) = dimensions {
        metrics.height = dimensions.height.unwrap_or(metrics.height);
    }
    metrics
}

pub(crate) fn flowchart_html_label_box(
    measurer: &dyn TextMeasurer,
    xhtml: &str,
) -> Option<HtmlLabelBox> {
    HtmlLabelBox::from_xhtml(xhtml, &mut || {
        (!measurer.cancellation_requested()).then_some(()).ok_or(())
    })
    .ok()
    .flatten()
}

#[cfg(feature = "markdown-labels")]
fn flowchart_html_label_content_metrics(
    request: FlowchartLabelMetricsRequest<'_>,
    xhtml: &str,
    geometry: HtmlTextGeometryDeclarations,
    authored_width: Option<HtmlLabelWidth>,
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
            measurer,
            xhtml,
            style,
            geometry,
            max_width_px,
            authored_width,
            fixed_img_width,
        )
    } else if has_inline_style
        || crate::text::flowchart_html_has_inline_style_tags(&lower)
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
    geometry: HtmlTextGeometryDeclarations,
    max_width_px: Option<f64>,
    authored_width: Option<HtmlLabelWidth>,
    fixed_img_width: bool,
) -> crate::text::TextMetrics {
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
                if let Some(natural) = intrinsic_label_image_size(src) {
                    let width = if fixed_img_width {
                        fallback_width
                    } else {
                        natural.width.min(max_width)
                    };
                    metrics.width = metrics.width.max(width);
                    // The image is fitted to the final label width after text is measured.
                    height_per_px += natural.height / natural.width;
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
    if flowchart_trim_html_collapsible_whitespace(text).is_empty() {
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
            .map(flowchart_trim_html_collapsible_whitespace)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_matches(is_html_collapsible_ascii_whitespace)
            .to_string()
    }

    let text = normalize_text_block(text);
    if !text.is_empty() {
        let styled = (!geometry.is_empty())
            .then(|| {
                let fragment = crate::svg::escape_xml(&text).replace('\n', "<br />");
                crate::svg::measure_styled_html_label(
                    measurer, &fragment, style, geometry, wrap_width, false,
                )
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

fn intrinsic_label_image_size(src: &str) -> Option<kurbo::Size> {
    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    {
        crate::raster_data_url::intrinsic_raster_size(src)
    }
    #[cfg(not(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )))]
    {
        let _ = src;
        None
    }
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

#[cfg(feature = "markdown-labels")]
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

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
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

#[cfg(any(feature = "layout-dagre", test))]
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

fn starts_with_img_tag(input: &str) -> bool {
    starts_with_ascii_case_insensitive(input, b"<img")
        && input
            .as_bytes()
            .get(4)
            .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(*byte, b'/' | b'>'))
}

pub(crate) fn normalize_flowchart_img_tags(input: &str, fixed_width: bool) -> Cow<'_, str> {
    // Mermaid flowchart-v2 adds inline styles to `<img>` tags inside HTML labels to constrain
    // their layout. The SVG baseline uses XHTML, so we also self-close the tags later.
    if find_ascii_case_insensitive(input, b"<img").is_none() {
        return Cow::Borrowed(input);
    }

    let style = if fixed_width {
        "display: flex; flex-direction: column; min-width: 80px; max-width: 80px;"
    } else {
        "display: flex; flex-direction: column; width: 100%;"
    };

    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(offset) = input[cursor..].find('<') {
        let start = cursor + offset;
        out.push_str(&input[cursor..start]);
        let rest = &input[start..];
        if !rest.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic) {
            out.push('<');
            cursor = start + 1;
            continue;
        }
        let Some(end) = merman_render_core::svg_scan::find_tag_end(input, start) else {
            cursor = start;
            break;
        };
        cursor = end + 1;
        if !starts_with_img_tag(rest) {
            out.push_str(&input[start..cursor]);
            continue;
        }

        let tag = BytesStart::from_content(&input[start + 1..end], 3);
        let mut attrs = [None, None, None];
        let names = ["src", "width", "height"];
        for attr in tag.html_attributes().with_checks(false).flatten() {
            if let Some(index) = names
                .iter()
                .position(|name| attr.key.as_ref().eq_ignore_ascii_case(name))
                && attrs[index].is_none()
            {
                attrs[index] = Some(
                    merman_model::text::decode_html_entities_to_unicode(&attr.value).into_owned(),
                );
            }
        }
        out.push_str("<img");
        for (name, value) in names.into_iter().zip(attrs) {
            if let Some(value) = value.filter(|value| name != "src" || !value.trim().is_empty()) {
                let _ = write!(out, r#" {name}="{}""#, crate::svg::escape_attr(&value));
            }
        }
        let _ = write!(out, r#" style="{style}">"#);
    }
    out.push_str(&input[cursor..]);
    Cow::Owned(out)
}

pub(crate) fn is_single_img_label(label: &str) -> bool {
    let label = flowchart_trim_html_collapsible_whitespace(label);
    if !starts_with_img_tag(label) {
        return false;
    }
    let Some(end) = merman_render_core::svg_scan::find_tag_end(label, 0) else {
        return false;
    };
    flowchart_trim_html_collapsible_whitespace(&label[end + 1..]).is_empty()
}

#[cfg(feature = "markdown-labels")]
pub(crate) fn flowchart_label_html_source(
    label: &str,
    label_type: &str,
    config: &merman_model::presentation::PresentationConfig,
) -> String {
    if flowchart_label_text_is_empty_for_mode(label, true) {
        return String::new();
    }

    match label_type {
        "markdown" => {
            let decoded = crate::entities::decode_mermaid_entities_for_render_text(label);
            let decoded = if decoded.contains("\\\\") {
                Cow::Owned(decoded.replace("\\\\", "\\"))
            } else {
                decoded
            };
            let markdown_auto_wrap = config
                .as_value()
                .get("markdownAutoWrap")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let html_out = crate::text::mermaid_markdown_to_html_label_fragment(
                decoded.trim_end_matches('\n'),
                markdown_auto_wrap,
            );
            crate::text::replace_fontawesome_icons(flowchart_trim_html_collapsible_whitespace(
                &html_out,
            ))
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

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    #[test]
    fn node_minimum_excludes_circles_fixed_symbols_assets_and_empty_labels() {
        let config = crate::test_support::presentation_config(serde_json::json!({
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
        // Keep the headless fallback for images without intrinsic metadata, and only use
        // actual img elements and src attributes from the sanitized label.
        let cases: &[(&str, &[&str], usize, f64)] = &[
            ("<img src='x'>", &[], 1, 80.0),
            ("<IMG src=''>", &[], 0, 80.0),
            (
                "<span>中😀</span><IMGx src='not-an-image'><IMG src='图'>尾",
                &["中😀", "尾"],
                1,
                120.0,
            ),
            ("a<im😀>b<img src='x'>", &["ab"], 1, 120.0),
            ("a<img😀 src='x'><br中>b<img src='x'>", &["ab"], 1, 120.0),
            ("a<img>", &["a"], 0, 120.0),
            ("a<img src='  '>", &["a"], 0, 120.0),
            ("a<img src=x>", &["a"], 1, 120.0),
            ("a<img src = 'x'>", &["a"], 1, 120.0),
            (
                "<span style='min-width: 80px;'>long text</span><img src='x'>",
                &["long text"],
                1,
                120.0,
            ),
            ("a<img data-src='x'>", &["a"], 0, 120.0),
            ("a<img src='a>b'>尾", &["a", "尾"], 1, 120.0),
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
        let config = crate::test_support::presentation_config(
            serde_json::json!({ "securityLevel": "strict" }),
        );
        let html = flowchart_label_html_source("head<img", "html", &config);
        let xhtml = merman_markup::sanitize::sanitize_xhtml(&html, config.as_config());
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
    fn strict_math_default_is_flowchart_local_and_preserves_frozen_declarations() {
        use std::sync::Arc;

        #[derive(Debug)]
        struct ConfigCheckingMathRenderer<'a> {
            original: &'a PresentationConfig,
            security: Option<&'static str>,
        }

        impl MathRenderer for ConfigCheckingMathRenderer<'_> {
            fn render_html_label(&self, _: &str, _: &PresentationConfig) -> Option<String> {
                unreachable!("the measurement callback handles this fixture")
            }

            fn measure_html_label(
                &self,
                _: &str,
                config: &PresentationConfig,
                _: &TextStyle,
                _: Option<f64>,
                _: WrapMode,
            ) -> Option<TextMetrics> {
                assert_eq!(config.get_str("securityLevel"), self.security);
                assert!(Arc::ptr_eq(
                    config.shared_explicit_config_paths(),
                    self.original.shared_explicit_config_paths(),
                ));
                assert!(config.explicit_config_owns_path("fontFamily"));
                assert_eq!(config.get_str("fontFamily"), Some("serif"));
                assert!(!config.explicit_config_owns_path("securityLevel"));
                Some(TextMetrics {
                    width: 123.5,
                    height: 20.0,
                    line_count: 1,
                })
            }
        }

        let config = crate::test_support::presentation_config(serde_json::json!({
            "fontFamily": "serif",
        }));
        let style = TextStyle::default();
        let generic =
            crate::math::math_label_metrics_for_layout(crate::math::MathLabelMetricsRequest {
                measurer: &PreciseTextMeasurer,
                raw_label: "$$x$$",
                style: &style,
                max_width_px: None,
                wrap_mode: WrapMode::HtmlLike,
                config: &config,
                math_renderer: Some(&ConfigCheckingMathRenderer {
                    original: &config,
                    security: None,
                }),
            })
            .expect("generic math keeps the caller's configuration");
        let flowchart = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: &PreciseTextMeasurer,
            raw_label: "$$x$$",
            label_type: "text",
            style: &style,
            max_width_px: None,
            wrap_mode: WrapMode::HtmlLike,
            config: &config,
            math_renderer: Some(&ConfigCheckingMathRenderer {
                original: &config,
                security: Some("strict"),
            }),
        });
        assert_eq!(generic.width, 123.5);
        assert_eq!(flowchart.width, 123.5);
        assert_eq!(config.get_str("securityLevel"), None);
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
        let config = crate::test_support::presentation_config(serde_json::json!({
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
        let config = crate::test_support::presentation_config(serde_json::json!({
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
        let config = crate::test_support::presentation_config(serde_json::json!({
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
