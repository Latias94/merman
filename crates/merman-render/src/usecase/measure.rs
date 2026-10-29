use super::*;
use crate::Error;
use crate::layout_work::OperationLayoutWorkControl;
use crate::text::{
    WrapMode, measure_wrapped_markdown_with_inline_styles, measure_xhtml_label_fragment,
};
use merman_core::diagrams::usecase::{
    UsecaseArrowType, UsecaseJsonInfinity, UsecaseJsonNode, UsecaseNodeKind,
    UsecaseRelationshipType,
};
use std::collections::HashSet;

#[derive(Debug)]
pub(crate) struct UsecasePreparedHtmlLabel {
    pub(crate) xhtml: String,
    _retained_text: crate::resources::PreparedTextRetainedReservation,
}

impl UsecasePreparedHtmlLabel {
    fn new(xhtml: String, work: &OperationLayoutWorkControl) -> Result<Arc<Self>> {
        let bytes = xhtml
            .capacity()
            .saturating_add(std::mem::size_of::<Self>())
            .saturating_add(2 * std::mem::size_of::<usize>());
        let retained_text = work.reserve_prepared_text_retained_bytes(bytes)?;
        Ok(Arc::new(Self {
            xhtml,
            _retained_text: retained_text,
        }))
    }
}

pub(super) fn number(config: &Value, key: &str, fallback: f64) -> f64 {
    config
        .get("usecase")
        .and_then(|value| value.get(key))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(fallback)
}

pub(super) fn text_style(config: &Value, actor: Option<bool>) -> TextStyle {
    let Some(actor) = actor else {
        return TextStyle {
            font_family: config
                .pointer("/themeVariables/fontFamily")
                .or_else(|| config.get("fontFamily"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            font_size: config
                .pointer("/themeVariables/fontSize")
                .and_then(|value| {
                    value
                        .as_f64()
                        .or_else(|| value.as_str()?.trim_end_matches("px").parse().ok())
                })
                .unwrap_or(16.0),
            ..Default::default()
        };
    };
    let prefix = if actor { "actor" } else { "usecase" };
    let string = |suffix: &str, fallback: &str| {
        config
            .get("usecase")
            .and_then(|value| value.get(format!("{prefix}{suffix}")))
            .and_then(Value::as_str)
            .unwrap_or(fallback)
            .to_owned()
    };
    TextStyle {
        font_family: Some(string("FontFamily", "\"Open Sans\", sans-serif")),
        font_size: number(
            config,
            &format!("{prefix}FontSize"),
            if actor { 14.0 } else { 12.0 },
        ),
        font_weight: Some(string("FontWeight", "normal")),
        font_style: None,
    }
}

fn styles(
    model: &UsecaseDiagramModel,
    classes: &[String],
    inline: &[String],
) -> indexmap::IndexMap<String, String> {
    let mut values = indexmap::IndexMap::new();
    for name in std::iter::once("default").chain(classes.iter().map(String::as_str)) {
        if let Some(definition) = model
            .class_defs
            .iter()
            .find(|definition| definition.id == name)
        {
            for item in &definition.styles {
                if let Some((key, value)) = crate::mermaid_style::parse_safe_style_decl(item) {
                    values.insert(key.to_owned(), value.to_owned());
                }
            }
        }
    }
    apply_inline_styles(&mut values, inline);
    values
}

fn apply_inline_styles(values: &mut indexmap::IndexMap<String, String>, inline: &[String]) {
    for item in inline {
        if let Some((key, value)) = crate::mermaid_style::parse_safe_style_decl(item) {
            values.insert(key.to_owned(), value.to_owned());
        }
    }
}

fn shape_style(styles: &indexmap::IndexMap<String, String>) -> String {
    styles
        .iter()
        .filter(|(key, _)| !crate::mermaid_style::is_label_style_key(key))
        .map(|(key, value)| {
            format!(
                "{key}:{} !important;",
                value.trim_end_matches("!important").trim()
            )
        })
        .collect()
}

fn styled(
    inherited: &TextStyle,
    element_defaults: Option<&TextStyle>,
    styles: &indexmap::IndexMap<String, String>,
    root_font_size: f64,
) -> TextStyle {
    let mut cascade = crate::mermaid_style::TextStyleCascade::new(
        inherited,
        crate::mermaid_style::CssFontSizeContext::new(inherited.font_size, root_font_size),
    );
    // Role selectors match this element, not its parent. Their defaults compete
    // with source declarations without changing the inherited relative-unit base.
    if let Some(defaults) = element_defaults {
        if let Some(family) = &defaults.font_family {
            cascade.observe("font-family", family, false);
        }
        cascade.observe("font-size", &format!("{}px", defaults.font_size), false);
        if let Some(weight) = &defaults.font_weight {
            cascade.observe("font-weight", weight, false);
        }
        if let Some(style) = &defaults.font_style {
            cascade.observe("font-style", style, false);
        }
    }
    for (key, value) in styles {
        let value = value.trim_end_matches("!important").trim();
        // The writer emits every compiled label declaration as important.
        cascade.observe(key, value, true);
    }
    cascade.finish().into_owned()
}

struct LabelContext<'a> {
    config: &'a merman_core::presentation::PresentationConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
    styles: &'a indexmap::IndexMap<String, String>,
    root_font_size: f64,
}

fn measure_plain_label(
    source: &str,
    style: &TextStyle,
    measurer: &dyn TextMeasurer,
) -> TextMetrics {
    // Match the writer's source tokenization before decoding each textContent payload.
    // Decoded rows are literal tspan text, not another input to label normalization.
    let lines = crate::text::DeterministicTextMeasurer::normalized_text_lines(source);
    let mut metrics = TextMetrics {
        width: 0.0,
        height: 0.0,
        line_count: lines.len(),
    };
    let mut vertical_bounds: Option<(f64, f64)> = None;
    for (index, line) in lines.iter().enumerate() {
        let visible = crate::text::non_markdown_svg_words(line)
            .map(crate::entities::decode_svg_text_content_entities)
            .collect::<Vec<_>>()
            .join(" ");
        if visible.is_empty() {
            continue;
        }
        metrics.width = metrics
            .width
            .max(measurer.measure_svg_tspan_text_bbox_width_px(&visible, style));
        let top = index as f64 * style.font_size * 1.1
            + measurer.measure_svg_create_text_bbox_y_offset_px(&visible, style);
        let bottom = top + measurer.measure_svg_tspan_text_bbox_height_px(&visible, style);
        vertical_bounds = Some(match vertical_bounds {
            Some((min, max)) => (min.min(top), max.max(bottom)),
            None => (top, bottom),
        });
    }
    metrics.height = vertical_bounds.map_or(0.0, |(top, bottom)| bottom - top);
    metrics
}

fn label(
    text: &str,
    label_type: UsecaseLabelType,
    style: &TextStyle,
    max_width: Option<f64>,
    min_width: f64,
    context: &LabelContext<'_>,
    work: &OperationLayoutWorkControl,
) -> Result<UsecaseLabelPlan> {
    let html = context
        .config
        .as_value()
        .get("htmlLabels")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mode = if html {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    // Node `style` describes g.label; edge groups carry no inline font. Source
    // styles then repeat on div/span in HTML or text in SVG, whose words reset
    // inherited weight and style.
    let mut terminal_style = styled(style, None, context.styles, context.root_font_size);
    if html {
        terminal_style = styled(
            &terminal_style,
            None,
            context.styles,
            context.root_font_size,
        );
    } else {
        terminal_style = crate::text::svg_markdown_word_style(
            &terminal_style,
            crate::text::MermaidMarkdownWordType::Normal,
        )
        .into_owned();
    }
    let source = create_text_source(text, label_type);
    let source = source.as_ref();
    let math_renderer = context
        .math_renderer
        .filter(|_| crate::math::contains_delimited_math(source));
    let html_content = if html {
        Some(
            crate::math::render_math_html_label(source, context.config, math_renderer)
                .unwrap_or_else(|| match label_type {
                    UsecaseLabelType::Text => {
                        format!("<p>{}</p>", source.replace('\n', "<br/>"))
                    }
                    UsecaseLabelType::Markdown => {
                        crate::text::mermaid_markdown_to_xhtml_label_fragment(
                            text,
                            crate::config::config_bool(
                                context.config.as_value(),
                                &["markdownAutoWrap"],
                            )
                            .unwrap_or(true),
                        )
                    }
                }),
        )
    } else {
        None
    };
    // The generated NBSP terminal has no automatic space breaks. Authored <br/> remains
    // structural XHTML and is still measured as separate rows with no width constraint.
    let measurement_width = if html
        && label_type == UsecaseLabelType::Markdown
        && crate::config::config_bool(context.config.as_value(), &["markdownAutoWrap"])
            == Some(false)
    {
        None
    } else {
        max_width
    };
    let mut metrics =
        crate::math::math_label_metrics_for_layout(crate::math::MathLabelMetricsRequest {
            measurer: context.measurer,
            raw_label: source,
            style: &terminal_style,
            max_width_px: measurement_width,
            wrap_mode: mode,
            config: context.config,
            math_renderer,
        })
        .unwrap_or_else(|| {
            if let Some(content) = html_content.as_deref() {
                measure_xhtml_label_fragment(
                    context.measurer,
                    content,
                    &terminal_style,
                    measurement_width,
                    WrapMode::HtmlLike,
                )
            } else if label_type == UsecaseLabelType::Markdown {
                measure_wrapped_markdown_with_inline_styles(
                    context.measurer,
                    text,
                    &terminal_style,
                    max_width,
                    mode,
                )
            } else {
                measure_plain_label(source, &terminal_style, context.measurer)
            }
        });
    let text_metrics = metrics;
    // labelHelper.withMinWidth also widens the measured box for SVG labels.
    if !text.is_empty() {
        metrics.width = metrics.width.max(min_width);
    }
    Ok(UsecaseLabelPlan {
        text: text.to_owned(),
        label_type,
        text_metrics,
        metrics,
        // HTML emits the g font again; retain it rather than re-emitting the
        // terminal size and applying relative descendant declarations twice.
        style: if html { style.clone() } else { terminal_style },
        max_width,
        html: html_content
            .map(|content| UsecasePreparedHtmlLabel::new(content, work))
            .transpose()?,
        styles: context
            .styles
            .iter()
            .filter(|(key, _)| crate::mermaid_style::is_label_style_key(key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    })
}

fn json_display_string(node: &UsecaseJsonNode, value: &str) -> Result<String> {
    node.display_string(value)
        .map(|text| text.into_owned())
        .map_err(|error| Error::InvalidModel {
            message: format!("Invalid Usecase JSON string encoding: {error}"),
        })
}

fn json_scalar_display(
    node: &UsecaseJsonNode,
    value: &Value,
    infinity: Option<&UsecaseJsonInfinity>,
) -> Result<String> {
    if value.is_null() {
        match infinity {
            Some(UsecaseJsonInfinity::Positive) => return Ok("Infinity".into()),
            Some(UsecaseJsonInfinity::Negative) => return Ok("-Infinity".into()),
            None => {}
        }
    }
    Ok(match value {
        Value::String(value) => return json_display_string(node, value),
        Value::Number(number) => number.as_f64().map_or_else(
            || number.to_string(),
            |value| ryu_js::Buffer::new().format(value).to_string(),
        ),
        _ => value.to_string(),
    })
}

fn flatten_json(
    node: &UsecaseJsonNode,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<(String, String, String)>> {
    // Iterative traversal bounds stack use for externally constructed render models.
    let mut pending = vec![(&node.value, String::new(), String::new())];
    let mut rows = Vec::new();
    while let Some((value, path, pointer)) = pending.pop() {
        work.charge_adapter(1)?;
        match value {
            Value::Array(values)
                if !values.is_empty()
                    && values
                        .iter()
                        .all(|value| !value.is_array() && !value.is_object()) =>
            {
                for (index, value) in values.iter().enumerate() {
                    work.charge_adapter(1)?;
                    let display = if index == 0 {
                        path.clone()
                    } else {
                        String::new()
                    };
                    let value = json_scalar_display(
                        node,
                        value,
                        node.non_finite_numbers.get(&format!("{pointer}/{index}")),
                    )?;
                    rows.push((display, path.clone(), value));
                }
            }
            Value::Array(values) if !values.is_empty() => {
                for (index, child) in values.iter().enumerate().rev() {
                    pending.push((
                        child,
                        format!("{path}[{index}]"),
                        format!("{pointer}/{index}"),
                    ));
                }
            }
            Value::Object(values) if !values.is_empty() => {
                let keys = node
                    .property_order
                    .get(&pointer)
                    .cloned()
                    .unwrap_or_else(|| values.keys().cloned().collect());
                for key in keys.into_iter().rev() {
                    if let Some(child) = values.get(&key) {
                        // Keep encoded identity for lookup and pointers; only the
                        // final UTF-8 SVG presentation replaces isolated surrogates.
                        let display_key = json_display_string(node, &key)?;
                        let child_path = if path.is_empty() {
                            display_key
                        } else {
                            format!("{path}.{display_key}")
                        };
                        let escaped = key.replace('~', "~0").replace('/', "~1");
                        pending.push((child, child_path, format!("{pointer}/{escaped}")));
                    }
                }
            }
            _ => rows.push((
                path.clone(),
                path,
                json_scalar_display(node, value, node.non_finite_numbers.get(&pointer))?,
            )),
        }
    }
    Ok(rows)
}

fn escaped_markdown(text: &str) -> String {
    let mut escaped = String::new();
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '*' | '[' | '\\' | ']' | '_' | '`' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub(super) fn measure(
    model: &UsecaseDiagramModel,
    full_config: &merman_core::presentation::PresentationConfig,
    css_binding: &UsecaseCssBinding,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn crate::math::MathRenderer + Send + Sync)>,
    work: &mut OperationLayoutWorkControl,
) -> Result<(Vec<UsecaseNodePlan>, Vec<UsecaseEdgePlan>)> {
    let config = full_config.as_value();
    let sanitize = |text: &str| merman_core::sanitize::sanitize_text(text, full_config.as_config());
    let min_width = number(config, "minNodeWidth", 120.0);
    let wrapping_width = Some(number(config, "wrappingWidth", 120.0));
    let html = config
        .get("htmlLabels")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let root_font_size = css_binding.root_font.font_size;
    let mut nodes = Vec::new();
    for actor in [true, false] {
        for node in model
            .nodes
            .iter()
            .filter(|node| (node.kind == UsecaseNodeKind::Actor) == actor)
        {
            work.charge_adapter(1)?;
            let css = styles(model, &node.classes, &node.styles);
            let role_font = if actor {
                &css_binding.actor_font
            } else {
                &css_binding.usecase_font
            };
            let (inherited, element_defaults) = if html {
                // HTML emits the computed role font inline on g.label.
                (role_font, None)
            } else {
                // Only SVG actor/business groups match a role selector; ordinary
                // SVG labels inherit the root without .nodeLabel/.usecase-label.
                let role_group =
                    actor || (node.business && node.kind == UsecaseNodeKind::UseCaseEllipse);
                (&css_binding.root_font, role_group.then_some(role_font))
            };
            let style = styled(inherited, element_defaults, &css, root_font_size);
            let label_context = LabelContext {
                config: full_config,
                measurer,
                math_renderer,
                styles: &css,
                root_font_size,
            };
            // Sanitize before escaping or folding stereotypes, as UsecaseDB does.
            let source_label = sanitize(&node.label);
            let source_stereotype = node.stereotype.as_deref().map(sanitize);
            let folded_stereotype = !actor
                && (node.kind == UsecaseNodeKind::UseCaseRect || !node.business)
                && source_stereotype
                    .as_ref()
                    .is_some_and(|value| !value.is_empty());
            let folded_text = source_stereotype
                .as_ref()
                .filter(|_| folded_stereotype)
                .map(|stereotype| {
                    let body = if node.label_type == UsecaseLabelType::Text {
                        escaped_markdown(&source_label)
                    } else {
                        source_label.clone()
                    };
                    format!("«{}»<br/>{body}", escaped_markdown(stereotype))
                });
            let main = label(
                folded_text.as_deref().unwrap_or(&source_label),
                if folded_stereotype {
                    UsecaseLabelType::Markdown
                } else {
                    node.label_type
                },
                &style,
                wrapping_width,
                if actor { 0.0 } else { min_width },
                &label_context,
                work,
            )?;
            let stereotype = source_stereotype
                .as_ref()
                .filter(|value| !value.is_empty() && !folded_stereotype)
                .map(|value| {
                    // The separate SVG stereotype lacks .actor-label/.usecase-label.
                    let stereotype_style = if html {
                        style.clone()
                    } else {
                        styled(&css_binding.root_font, None, &css, root_font_size)
                    };
                    label(
                        &format!("«{value}»"),
                        UsecaseLabelType::Text,
                        &stereotype_style,
                        wrapping_width,
                        0.0,
                        &label_context,
                        work,
                    )
                })
                .transpose()?;
            let stereo_width = stereotype.as_ref().map_or(0.0, |value| value.metrics.width);
            let stereo_height = stereotype
                .as_ref()
                .map_or(0.0, |value| value.metrics.height);
            let label_width = main.metrics.width.max(stereo_width);
            let separate_stereotype =
                actor || (node.kind == UsecaseNodeKind::UseCaseEllipse && node.business);
            let content_height = main.metrics.height
                + stereo_height
                + if stereotype.is_some() && separate_stereotype {
                    2.0
                } else {
                    0.0
                };
            let ellipse = node.kind == UsecaseNodeKind::UseCaseEllipse;
            let (width, height) = if actor {
                // usecaseActor.ts: one shared 56x72 glyph footprint for all four variants.
                (
                    56.0_f64.max(label_width) + 20.0,
                    72.0 + 8.0 + content_height + 20.0,
                )
            } else if ellipse {
                (label_width + 40.0, content_height + 40.0)
            } else if config.get("look").and_then(Value::as_str).unwrap_or("neo") == "neo" {
                (label_width + 32.0, content_height + 24.0)
            } else {
                (label_width + 40.0, content_height + 20.0)
            };
            nodes.push(UsecaseNodePlan {
                id: node.id.clone(),
                source_style: shape_style(&css),
                parent: node.parent_id.clone(),
                source_label,
                label: main,
                stereotype,
                folded_stereotype,
                width,
                height,
                is_boundary: false,
                package: false,
                ellipse,
                table: None,
                dagre_helper: false,
            });
        }
    }
    let generic = &css_binding.generic_font;
    for note in &model.notes {
        work.charge_adapter(1)?;
        let css = styles(model, &[], &[]);
        let style = styled(generic, None, &css, root_font_size);
        let label_context = LabelContext {
            config: full_config,
            measurer,
            math_renderer,
            styles: &css,
            root_font_size,
        };
        let main = label(
            &sanitize(&note.label),
            note.label_type,
            &style,
            wrapping_width,
            min_width,
            &label_context,
            work,
        )?;
        nodes.push(UsecaseNodePlan {
            id: note.id.clone(),
            source_style: shape_style(&css),
            parent: None,
            width: main.metrics.width + 20.0,
            height: main.metrics.height + 20.0,
            source_label: main.text.clone(),
            label: main,
            stereotype: None,
            folded_stereotype: false,
            is_boundary: false,
            package: false,
            ellipse: false,
            table: None,
            dagre_helper: false,
        });
    }
    for node in &model.json_nodes {
        work.charge_adapter(1)?;
        let css = styles(model, &node.classes, &node.styles);
        let style = styled(generic, None, &css, root_font_size);
        let label_context = LabelContext {
            config: full_config,
            measurer,
            math_renderer,
            styles: &css,
            root_font_size,
        };
        let main = label(
            &sanitize(&node.id),
            UsecaseLabelType::Text,
            &style,
            wrapping_width,
            min_width,
            &label_context,
            work,
        )?;
        let mut rows = Vec::new();
        let mut key_width: f64 = 16.0;
        let mut value_width: f64 = 16.0;
        for (key, accessible_key, value) in flatten_json(node, work)? {
            work.charge_adapter(1)?;
            let key_label = label(
                &sanitize(&key),
                UsecaseLabelType::Text,
                &style,
                None,
                0.0,
                &label_context,
                work,
            )?;
            let value_label = label(
                &sanitize(&value),
                UsecaseLabelType::Text,
                &style,
                None,
                0.0,
                &label_context,
                work,
            )?;
            key_width = key_width.max(key_label.metrics.width + 16.0);
            value_width = value_width.max(value_label.metrics.width + 16.0);
            let height = key_label.metrics.height.max(value_label.metrics.height) + 8.0;
            rows.push(UsecaseJsonRowPlan {
                key: key_label,
                value: value_label,
                accessible_key: sanitize(&accessible_key),
                height,
            });
        }
        let inner_width = (main.metrics.width + 16.0).max(key_width + value_width);
        value_width += (inner_width - key_width - value_width).max(0.0);
        let title_height = main.metrics.height + 8.0;
        let inner_height = title_height + rows.iter().map(|row| row.height).sum::<f64>();
        let border_width = css
            .get("stroke-width")
            .and_then(|value| value.trim_end_matches("px").parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(1.0);
        nodes.push(UsecaseNodePlan {
            id: node.id.clone(),
            source_style: shape_style(&css),
            parent: None,
            source_label: main.text.clone(),
            label: main,
            stereotype: None,
            folded_stereotype: false,
            width: inner_width + border_width * 2.0,
            height: inner_height + border_width * 2.0,
            is_boundary: false,
            package: false,
            ellipse: false,
            dagre_helper: false,
            table: Some(UsecaseJsonTablePlan {
                rows,
                key_width,
                value_width,
                title_height,
                border_width,
            }),
        });
    }
    for boundary in &model.boundaries {
        work.charge_adapter(1)?;
        let css = styles(model, &boundary.classes, &boundary.styles);
        let style = styled(generic, None, &css, root_font_size);
        let label_context = LabelContext {
            config: full_config,
            measurer,
            math_renderer,
            styles: &css,
            root_font_size,
        };
        let main = label(
            &sanitize(&boundary.label),
            boundary.label_type,
            &style,
            Some(200.0),
            0.0,
            &label_context,
            work,
        )?;
        nodes.push(UsecaseNodePlan {
            id: boundary.id.clone(),
            source_style: shape_style(&css),
            parent: None,
            width: main.metrics.width + 20.0,
            height: main.metrics.height + if boundary.package { 50.0 } else { 40.0 },
            source_label: main.text.clone(),
            label: main,
            stereotype: None,
            folded_stereotype: false,
            is_boundary: true,
            package: boundary.package,
            ellipse: false,
            table: None,
            dagre_helper: false,
        });
    }
    let mut edges = Vec::new();
    for edge in &model.relationships {
        work.charge_adapter(1)?;
        let mut css = styles(model, &edge.classes, &[]);
        // Mermaid selects Neo marker margins from class styles before inline edge styles.
        let source_animated = css
            .iter()
            .any(|(key, value)| key.contains("animation") || value.contains("animation"));
        apply_inline_styles(&mut css, &edge.styles);
        let style = generic.clone();
        let label_context = LabelContext {
            config: full_config,
            measurer,
            math_renderer,
            styles: &css,
            root_font_size,
        };
        let (text, kind, start, end, dotted) = match edge.relationship_type {
            UsecaseRelationshipType::Include => (
                Some("include"),
                UsecaseLabelType::Text,
                None,
                Some("arrow_point"),
                true,
            ),
            UsecaseRelationshipType::Extend => (
                Some("extend"),
                UsecaseLabelType::Text,
                None,
                Some("arrow_point"),
                true,
            ),
            UsecaseRelationshipType::Generalization => {
                (None, UsecaseLabelType::Text, None, Some("extension"), false)
            }
            UsecaseRelationshipType::Association => {
                let (start, end) = match edge.arrow_type {
                    UsecaseArrowType::Point => (None, Some("arrow_point")),
                    UsecaseArrowType::ReversedPoint => (Some("arrow_point"), None),
                    UsecaseArrowType::Circle => (None, Some("arrow_circle")),
                    UsecaseArrowType::ReversedCircle => (Some("arrow_circle"), None),
                    UsecaseArrowType::Cross => (None, Some("arrow_cross")),
                    UsecaseArrowType::ReversedCross => (Some("arrow_cross"), None),
                    UsecaseArrowType::Line => (None, None),
                };
                (
                    edge.label.as_deref().filter(|value| !value.is_empty()),
                    edge.label_type.unwrap_or_default(),
                    start,
                    end,
                    false,
                )
            }
        };
        edges.push(UsecaseEdgePlan {
            source_style: shape_style(&css),
            // Mermaid's marker margin decision observes compiled class animation, excluding
            // inline path styles. Prepare that decision alongside the edge's other source facts.
            animated: edge.animate || edge.animation.is_some() || source_animated,
            original_id: None,
            self_loop_node: None,
            dagre_recursive: false,
            id: edge.id.clone(),
            source: edge.source.clone(),
            target: edge.target.clone(),
            label: text
                .map(|text| {
                    let mut plan = label(
                        &sanitize(text),
                        kind,
                        &style,
                        Some(200.0),
                        0.0,
                        &label_context,
                        work,
                    )?;
                    if full_config
                        .as_value()
                        .get("htmlLabels")
                        .and_then(Value::as_bool)
                        == Some(false)
                        && !plan.text.is_empty()
                    {
                        // insertEdgeLabel passes createText's padded background bbox to both
                        // graph providers and the label-aware ELK terminal guard.
                        plan.metrics.width += 4.0;
                        plan.metrics.height += 4.0;
                    }
                    Ok::<_, crate::Error>(plan)
                })
                .transpose()?,
            minlen: edge.minlen,
            start_marker: start.map(str::to_owned),
            end_marker: end.map(str::to_owned),
            dotted,
            internal: false,
        });
    }
    for note in &model.notes {
        work.charge_adapter(1)?;
        edges.push(UsecaseEdgePlan {
            source_style: String::new(),
            animated: false,
            original_id: None,
            self_loop_node: None,
            dagre_recursive: false,
            id: format!("{}-edge", note.id),
            source: note.id.clone(),
            target: note.target.clone(),
            label: None,
            minlen: 1,
            start_marker: None,
            end_marker: None,
            dotted: true,
            internal: true,
        });
    }
    let ids: HashSet<_> = nodes.iter().map(|node| node.id.as_str()).collect();
    if ids.len() != nodes.len() {
        return Err(Error::InvalidModel {
            message: "duplicate Usecase node id".into(),
        });
    }
    for node in &nodes {
        if let Some(parent) = &node.parent
            && !model
                .boundaries
                .iter()
                .any(|boundary| &boundary.id == parent)
        {
            return Err(Error::InvalidModel {
                message: format!("missing Usecase boundary {parent}"),
            });
        }
    }
    for edge in &edges {
        if !ids.contains(edge.source.as_str()) || !ids.contains(edge.target.as_str()) {
            return Err(Error::InvalidModel {
                message: format!(
                    "missing Usecase edge endpoint: {} -> {}",
                    edge.source, edge.target
                ),
            });
        }
    }
    Ok((nodes, edges))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;
    use crate::text::DeterministicTextMeasurer;
    use merman_core::diagrams::usecase::{
        UsecaseActorType, UsecaseBoundary, UsecaseClassDef, UsecaseNode, UsecaseNote,
        UsecaseRelationship,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn model() -> UsecaseDiagramModel {
        UsecaseDiagramModel {
            direction: "LR".into(),
            nodes: Vec::new(),
            boundaries: Vec::new(),
            relationships: Vec::new(),
            notes: Vec::new(),
            json_nodes: Vec::new(),
            class_defs: Vec::new(),
            title: None,
            acc_title: None,
            acc_description: None,
        }
    }

    fn work() -> OperationLayoutWorkControl {
        OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::default(),
        )))
    }

    fn styled_model() -> UsecaseDiagramModel {
        let mut model = model();
        model.class_defs = vec![
            UsecaseClassDef {
                id: "default".into(),
                styles: vec![
                    "fill:#111111".into(),
                    "stroke:#222222".into(),
                    "color:#123456 !important".into(),
                    "font-size:20px".into(),
                ],
            },
            UsecaseClassDef {
                id: "accent".into(),
                styles: vec!["stroke:#333333".into(), "stroke-width:2px".into()],
            },
        ];
        for id in ["A", "B"] {
            model.nodes.push(UsecaseNode {
                id: id.into(),
                label: id.into(),
                label_type: UsecaseLabelType::Text,
                kind: UsecaseNodeKind::UseCaseEllipse,
                actor_type: UsecaseActorType::Normal,
                icon: None,
                parent_id: None,
                business: false,
                stereotype: None,
                classes: vec!["accent".into()],
                styles: vec![
                    "fill:var(--panel) !important".into(),
                    "stroke-width:7px".into(),
                    "fill: red; stroke: blue".into(),
                ],
            });
        }
        model.relationships.push(UsecaseRelationship {
            id: "edge".into(),
            explicit_id: true,
            source: "A".into(),
            target: "B".into(),
            relationship_type: UsecaseRelationshipType::Association,
            arrow_type: UsecaseArrowType::Point,
            label: None,
            label_type: None,
            dotted: false,
            minlen: 1,
            classes: vec!["accent".into()],
            styles: vec!["stroke-width:7px !important".into()],
            animate: false,
            animation: None,
        });
        model
    }

    #[test]
    fn source_styles_are_prepared_for_shapes_boundaries_and_unlabelled_edges() {
        let mut model = styled_model();
        model.nodes[0].parent_id = Some("boundary".into());
        model.boundaries.push(UsecaseBoundary {
            id: "boundary".into(),
            label: "Boundary".into(),
            label_type: UsecaseLabelType::Text,
            members: vec!["A".into()],
            package: true,
            classes: vec!["accent".into()],
            styles: vec!["fill:#444444".into()],
        });
        model.notes.push(UsecaseNote {
            id: "note".into(),
            target: "A".into(),
            label: "Note".into(),
            label_type: UsecaseLabelType::Text,
        });
        model.json_nodes.push(UsecaseJsonNode {
            id: "json".into(),
            value: json!({"key": 1}),
            property_order: BTreeMap::new(),
            string_encoding: None,
            non_finite_numbers: BTreeMap::new(),
            classes: vec!["accent".into()],
            styles: Vec::new(),
        });
        for html in [false, true] {
            let config = json!({"htmlLabels": html});
            let (nodes, edges) = measure(
                &model,
                &merman_core::presentation::PresentationConfig::from_value(config.clone()),
                &UsecaseCssBinding::new(&config),
                &DeterministicTextMeasurer::default(),
                None,
                &mut work(),
            )
            .unwrap();
            let node = nodes.iter().find(|node| node.id == "A").unwrap();
            assert_eq!(
                node.source_style,
                "fill:var(--panel) !important;stroke:#333333 !important;stroke-width:7px !important;"
            );
            assert_eq!(node.label.styles["color"], "#123456 !important");
            assert_eq!(node.label.style.font_size, 20.0);
            assert!(!node.source_style.contains("color:"));
            assert!(!node.source_style.contains("font-size:"));
            let boundary = nodes.iter().find(|node| node.is_boundary).unwrap();
            assert_eq!(
                boundary.source_style,
                "fill:#444444 !important;stroke:#333333 !important;stroke-width:2px !important;"
            );
            assert_eq!(
                nodes
                    .iter()
                    .find(|node| node.id == "note")
                    .unwrap()
                    .source_style,
                "fill:#111111 !important;stroke:#222222 !important;"
            );
            assert_eq!(
                nodes
                    .iter()
                    .find(|node| node.id == "json")
                    .unwrap()
                    .source_style,
                "fill:#111111 !important;stroke:#333333 !important;stroke-width:2px !important;"
            );
            assert!(edges[0].label.is_none());
            assert_eq!(
                edges[0].source_style,
                "fill:#111111 !important;stroke:#333333 !important;stroke-width:7px !important;"
            );
            let note_edge = edges.iter().find(|edge| edge.internal).unwrap();
            assert!(note_edge.source_style.is_empty());
            assert!(note_edge.dotted);
        }
    }

    #[test]
    fn self_loop_projection_keeps_original_prepared_style_on_all_segments() {
        let mut model = styled_model();
        model.relationships[0].target = "A".into();
        let config = json!({});
        let css_binding = UsecaseCssBinding::new(&config);
        let mut work = work();
        let (mut nodes, mut edges) = measure(
            &model,
            &merman_core::presentation::PresentationConfig::from_value(config.clone()),
            &css_binding,
            &DeterministicTextMeasurer::default(),
            None,
            &mut work,
        )
        .unwrap();
        let style = edges[0].source_style.clone();
        super::super::dagre::expand_self_loops(
            &mut nodes,
            &mut edges,
            &css_binding.generic_font,
            &mut work,
        )
        .unwrap();
        assert_eq!(edges.len(), 3);
        for edge in &edges {
            assert_eq!(edge.original_id.as_deref(), Some("edge"));
            assert_eq!(edge.self_loop_node.as_deref(), Some("A"));
            assert_eq!(edge.source_style, style);
        }
        let helpers = nodes
            .iter()
            .filter(|node| node.dagre_helper)
            .collect::<Vec<_>>();
        assert_eq!(helpers.len(), 2);
        assert!(helpers.iter().all(|node| node.source_style.is_empty()));
    }

    #[test]
    fn json_number_rows_use_javascript_precision_and_exponent_notation() {
        let node = UsecaseJsonNode {
            id: "data".into(),
            value: json!({"values": [9007199254740993_u64, 1e20, 1e21, -0.0, 1e-7, 1e-6]}),
            property_order: BTreeMap::new(),
            string_encoding: None,
            non_finite_numbers: BTreeMap::new(),
            classes: Vec::new(),
            styles: Vec::new(),
        };
        let rows = flatten_json(&node, &mut work()).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.2.as_str()).collect::<Vec<_>>(),
            [
                "9007199254740992",
                "100000000000000000000",
                "1e+21",
                "0",
                "1e-7",
                "0.000001",
            ]
        );
    }

    #[test]
    fn actor_footprint_tracks_font_without_inheriting_box_minimum() {
        let mut model = model();
        model.nodes.push(UsecaseNode {
            id: "a".into(),
            label: "A".into(),
            label_type: UsecaseLabelType::Text,
            kind: UsecaseNodeKind::Actor,
            actor_type: UsecaseActorType::Icon,
            icon: Some("fa:user".into()),
            parent_id: None,
            business: false,
            stereotype: None,
            classes: Vec::new(),
            styles: Vec::new(),
        });
        let measurer = DeterministicTextMeasurer::default();
        let (small, _) = measure(
            &model,
            &merman_core::presentation::PresentationConfig::from_value(
                json!({"usecase": {"minNodeWidth": 900}}),
            ),
            &UsecaseCssBinding::new(&json!({"usecase": {"minNodeWidth": 900}})),
            &measurer,
            None,
            &mut work(),
        )
        .unwrap();
        let (large, _) = measure(
            &model,
            &merman_core::presentation::PresentationConfig::from_value(
                json!({"usecase": {"actorFontSize": 40}}),
            ),
            &UsecaseCssBinding::new(&json!({"usecase": {"actorFontSize": 40}})),
            &measurer,
            None,
            &mut work(),
        )
        .unwrap();
        assert_eq!(small[0].width, 76.0);
        assert!(large[0].height > small[0].height);
    }

    #[test]
    fn json_rows_preserve_nested_source_order_and_empty_containers() {
        let mut model = model();
        model.json_nodes.push(UsecaseJsonNode {
            id: "data".into(),
            value: json!({"a": 1, "z": {"second": [], "first": false}}),
            property_order: BTreeMap::from([
                (String::new(), vec!["z".into(), "a".into()]),
                ("/z".into(), vec!["second".into(), "first".into()]),
            ]),
            string_encoding: None,
            non_finite_numbers: BTreeMap::new(),
            classes: Vec::new(),
            styles: Vec::new(),
        });
        let (nodes, _) = measure(
            &model,
            &merman_core::presentation::PresentationConfig::from_value(json!({})),
            &UsecaseCssBinding::new(&json!({})),
            &DeterministicTextMeasurer::default(),
            None,
            &mut work(),
        )
        .unwrap();
        let table = nodes[0].table.as_ref().unwrap();
        assert_eq!(
            table
                .rows
                .iter()
                .map(|row| (row.accessible_key.as_str(), row.value.text.as_str()))
                .collect::<Vec<_>>(),
            [("z.second", "[]"), ("z.first", "false"), ("a", "1")]
        );
        assert!(nodes[0].width >= table.key_width + table.value_width);
        assert!(nodes[0].height > table.rows.iter().map(|row| row.height).sum::<f64>());
    }

    #[test]
    fn scalar_json_arrays_share_their_accessible_key() {
        let node = UsecaseJsonNode {
            id: "data".into(),
            value: json!({"roles": ["admin", "viewer"]}),
            property_order: BTreeMap::new(),
            string_encoding: None,
            non_finite_numbers: BTreeMap::new(),
            classes: Vec::new(),
            styles: Vec::new(),
        };
        assert_eq!(
            flatten_json(&node, &mut work()).unwrap(),
            [
                ("roles".into(), "roles".into(), "admin".into()),
                ("".into(), "roles".into(), "viewer".into()),
            ]
        );
    }
    #[test]
    fn json_infinities_retain_sign_in_scalar_arrays_and_nested_rows() {
        let node = UsecaseJsonNode {
            id: "data".into(),
            value: json!({"a/b": [null, null, null, "1e309"], "nested": [{"~key": null}], "null": null}),
            property_order: BTreeMap::new(),
            string_encoding: None,
            non_finite_numbers: BTreeMap::from([
                ("/a~1b/0".into(), UsecaseJsonInfinity::Positive),
                ("/a~1b/1".into(), UsecaseJsonInfinity::Negative),
                ("/nested/0/~0key".into(), UsecaseJsonInfinity::Negative),
            ]),
            classes: Vec::new(),
            styles: Vec::new(),
        };
        let rows = flatten_json(&node, &mut work()).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.2.as_str()).collect::<Vec<_>>(),
            [
                "Infinity",
                "-Infinity",
                "null",
                "1e309",
                "-Infinity",
                "null"
            ]
        );
    }
}
