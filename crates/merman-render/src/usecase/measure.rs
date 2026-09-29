use super::*;
use crate::Error;
use crate::layout_work::OperationLayoutWorkControl;
use crate::text::{WrapMode, measure_wrapped_markdown_with_inline_styles};
use merman_core::diagrams::usecase::{
    UsecaseArrowType, UsecaseJsonNode, UsecaseNodeKind, UsecaseRelationshipType,
};
use std::collections::HashSet;

pub(super) fn number(config: &Value, key: &str, fallback: f64) -> f64 {
    config
        .get("usecase")
        .and_then(|value| value.get(key))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(fallback)
}

pub(crate) fn text_style(config: &Value, actor: Option<bool>) -> TextStyle {
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

pub(crate) fn styles(
    model: &UsecaseDiagramRenderModel,
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
    for item in inline {
        if let Some((key, value)) = crate::mermaid_style::parse_safe_style_decl(item) {
            values.insert(key.to_owned(), value.to_owned());
        }
    }
    values
}

fn styled(mut base: TextStyle, styles: &indexmap::IndexMap<String, String>) -> TextStyle {
    for (key, value) in styles {
        let value = value.trim_end_matches("!important").trim();
        match key.as_str() {
            "font-family" => base.font_family = Some(value.to_owned()),
            "font-weight" => base.font_weight = Some(value.to_owned()),
            "font-style" => base.font_style = Some(value.to_owned()),
            "font-size" => {
                if let Ok(size) = value.trim_end_matches("px").parse::<f64>()
                    && size.is_finite()
                    && size > 0.0
                {
                    base.font_size = size;
                }
            }
            _ => {}
        }
    }
    base
}

struct LabelContext<'a> {
    config: &'a Value,
    measurer: &'a dyn TextMeasurer,
    styles: &'a indexmap::IndexMap<String, String>,
}

fn label(
    text: &str,
    label_type: UsecaseLabelType,
    style: &TextStyle,
    max_width: Option<f64>,
    min_width: f64,
    context: &LabelContext<'_>,
) -> UsecaseLabelPlan {
    let html = context
        .config
        .get("htmlLabels")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mode = if html {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    let mut metrics = if label_type == UsecaseLabelType::Markdown {
        measure_wrapped_markdown_with_inline_styles(context.measurer, text, style, max_width, mode)
    } else {
        // Plain Usecase labels are escaped before createText; markup is literal content.
        let escaped = normalize_plain_label_line_breaks(text)
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        context
            .measurer
            .measure_wrapped(&escaped, style, max_width, mode)
    };
    // labelHelper.withMinWidth also widens the measured box for SVG labels.
    if !text.is_empty() {
        metrics.width = metrics.width.max(min_width);
    }
    UsecaseLabelPlan {
        text: text.to_owned(),
        label_type,
        metrics,
        style: style.clone(),
        max_width,
        styles: context
            .styles
            .iter()
            .filter(|(key, _)| crate::mermaid_style::is_label_style_key(key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    }
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
                    let value = value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string());
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
                        let child_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{path}.{key}")
                        };
                        let escaped = key.replace('~', "~0").replace('/', "~1");
                        pending.push((child, child_path, format!("{pointer}/{escaped}")));
                    }
                }
            }
            Value::String(value) => rows.push((path.clone(), path, value.clone())),
            _ => rows.push((path.clone(), path, value.to_string())),
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
    model: &UsecaseDiagramRenderModel,
    config: &Value,
    measurer: &dyn TextMeasurer,
    work: &mut OperationLayoutWorkControl,
) -> Result<(Vec<UsecaseNodePlan>, Vec<UsecaseEdgePlan>)> {
    let sanitize_config = merman_core::MermaidConfig::from_value(config.clone());
    let sanitize = |text: &str| merman_core::sanitize::sanitize_text(text, &sanitize_config);
    let min_width = number(config, "minNodeWidth", 120.0);
    let wrapping_width = Some(number(config, "wrappingWidth", 120.0));
    let mut nodes = Vec::new();
    for actor in [true, false] {
        for node in model
            .nodes
            .iter()
            .filter(|node| (node.kind == UsecaseNodeKind::Actor) == actor)
        {
            work.charge_adapter(1)?;
            let css = styles(model, &node.classes, &node.styles);
            let style = styled(text_style(config, Some(actor)), &css);
            let label_context = LabelContext {
                config,
                measurer,
                styles: &css,
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
            );
            let stereotype = source_stereotype
                .as_ref()
                .filter(|value| !value.is_empty() && !folded_stereotype)
                .map(|value| {
                    label(
                        &format!("«{value}»"),
                        UsecaseLabelType::Text,
                        &style,
                        wrapping_width,
                        0.0,
                        &label_context,
                    )
                });
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
    let generic = text_style(config, None);
    for note in &model.notes {
        work.charge_adapter(1)?;
        let css = styles(model, &[], &[]);
        let style = styled(generic.clone(), &css);
        let label_context = LabelContext {
            config,
            measurer,
            styles: &css,
        };
        let main = label(
            &sanitize(&note.label),
            note.label_type,
            &style,
            wrapping_width,
            min_width,
            &label_context,
        );
        nodes.push(UsecaseNodePlan {
            id: note.id.clone(),
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
        let style = styled(generic.clone(), &css);
        let label_context = LabelContext {
            config,
            measurer,
            styles: &css,
        };
        let main = label(
            &sanitize(&node.id),
            UsecaseLabelType::Text,
            &style,
            wrapping_width,
            min_width,
            &label_context,
        );
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
            );
            let value_label = label(
                &sanitize(&value),
                UsecaseLabelType::Text,
                &style,
                None,
                0.0,
                &label_context,
            );
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
        let style = styled(generic.clone(), &css);
        let label_context = LabelContext {
            config,
            measurer,
            styles: &css,
        };
        let main = label(
            &sanitize(&boundary.label),
            boundary.label_type,
            &style,
            Some(200.0),
            0.0,
            &label_context,
        );
        nodes.push(UsecaseNodePlan {
            id: boundary.id.clone(),
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
        let css = styles(model, &edge.classes, &edge.styles);
        let style = styled(generic.clone(), &css);
        let label_context = LabelContext {
            config,
            measurer,
            styles: &css,
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
            original_id: None,
            self_loop_node: None,
            dagre_recursive: false,
            id: edge.id.clone(),
            source: edge.source.clone(),
            target: edge.target.clone(),
            label: text.map(|text| {
                label(
                    &sanitize(text),
                    kind,
                    &style,
                    Some(200.0),
                    0.0,
                    &label_context,
                )
            }),
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
    use merman_core::diagrams::usecase::{UsecaseActorType, UsecaseNode};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn model() -> UsecaseDiagramRenderModel {
        UsecaseDiagramRenderModel {
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
            &json!({"usecase": {"minNodeWidth": 900}}),
            &measurer,
            &mut work(),
        )
        .unwrap();
        let (large, _) = measure(
            &model,
            &json!({"usecase": {"actorFontSize": 40}}),
            &measurer,
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
            classes: Vec::new(),
            styles: Vec::new(),
        });
        let (nodes, _) = measure(
            &model,
            &json!({}),
            &DeterministicTextMeasurer::default(),
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
}
