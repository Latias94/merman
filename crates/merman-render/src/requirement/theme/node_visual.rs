use super::*;
use indexmap::IndexMap;
use std::fmt::Write;

#[derive(Debug)]
pub(crate) struct RequirementNodeVisual {
    pub(crate) label_styles: String,
    pub(crate) label_div_style_prefix: String,
    pub(crate) svg_text_styles: String,
    pub(crate) node_styles: String,
    pub(crate) source_text_color: Option<String>,
    pub(crate) source_owns_fill: bool,
    pub(crate) source_owns_stroke: bool,
    pub(crate) typed_fill: Option<(usize, String)>,
    pub(crate) typed_stroke: Option<(usize, String)>,
    pub(crate) fill: String,
    pub(crate) stroke: String,
    pub(crate) stroke_width: f64,
    pub(crate) fill_style_attr: String,
    pub(crate) stroke_style_declaration: Option<String>,
    pub(crate) stroke_style_attr: String,
}

#[derive(Debug)]
struct NodeStyleOverrides {
    label_styles: String,
    label_div_style_prefix: String,
    svg_text_styles: String,
    node_styles: String,
    fill: Option<String>,
    color: Option<String>,
    stroke: Option<String>,
    stroke_width: Option<f64>,
}

fn parse_node_style_overrides(css_styles: &[String], html_labels: bool) -> NodeStyleOverrides {
    // Mirror Mermaid `styles2String(node)` output:
    // - De-duplicate by key (`Map` semantics) while preserving first insertion order.
    // - Split into label vs node styles via Mermaid `isLabelStyle`.
    // - Append ` !important` when emitting style strings.
    fn is_label_style(key: &str) -> bool {
        matches!(
            key,
            "color"
                | "font-size"
                | "font-family"
                | "font-weight"
                | "font-style"
                | "text-decoration"
                | "text-align"
                | "text-transform"
                | "line-height"
                | "letter-spacing"
                | "word-spacing"
                | "text-shadow"
                | "text-overflow"
                | "white-space"
                | "word-wrap"
                | "word-break"
                | "overflow-wrap"
                | "hyphens"
        )
    }

    let mut styles: IndexMap<String, String> = IndexMap::new();
    for raw in css_styles {
        let Some(parsed) = crate::mermaid_style::parse_style_declaration(raw) else {
            continue;
        };
        let k = parsed.property().to_string();
        let v = parsed.value().to_string();

        // JS `Map#set` overwrites the value without changing the key order.
        if let Some(existing) = styles.get_mut(&k) {
            *existing = v;
        } else {
            styles.insert(k, v);
        }
    }

    let mut label_kv: Vec<(&str, &str)> = Vec::new();
    let mut node_kv: Vec<(&str, &str)> = Vec::new();
    for (k, v) in &styles {
        if is_label_style(k.trim().to_ascii_lowercase().as_str()) {
            label_kv.push((k.as_str(), v.as_str()));
        } else {
            node_kv.push((k.as_str(), v.as_str()));
        }
    }

    let label_styles = label_kv
        .iter()
        .map(|(k, v)| format!("{k}:{v} !important"))
        .collect::<Vec<_>>()
        .join(";");
    // createText converts color only on <text>; requirementBox retains the
    // original declarations on the label group and first-row inner tspans.
    let svg_text_styles = if html_labels {
        String::new()
    } else {
        label_kv
            .iter()
            .map(|(key, value)| {
                let key = if *key == "color" { "fill" } else { key };
                format!("{key}:{value} !important")
            })
            .collect::<Vec<_>>()
            .join(";")
    };
    let label_div_style_prefix = label_kv
        .iter()
        .map(|(k, v)| format!("{k}: {v} !important; "))
        .collect::<Vec<_>>()
        .join("");
    let node_styles = node_kv
        .iter()
        .map(|(k, v)| format!("{k}:{v} !important"))
        .collect::<Vec<_>>()
        .join(";");

    let fill = styles.get("fill").cloned();
    let color = styles.get("color").cloned();
    let stroke = styles.get("stroke").cloned();
    let stroke_width = styles
        .get("stroke-width")
        .and_then(|v| v.trim_end_matches("px").trim().parse::<f64>().ok());

    NodeStyleOverrides {
        label_styles,
        label_div_style_prefix,
        svg_text_styles,
        node_styles,
        fill,
        color,
        stroke,
        stroke_width,
    }
}

impl RequirementPaintThemePlan {
    pub(super) fn prepare_node_visuals(
        mut self,
        model: &RequirementDiagramRenderModel,
        config: &MermaidConfig,
        work: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let config = config.as_value();
        let html = crate::config::config_bool(config, &["htmlLabels"]).unwrap_or(true);
        let hand_drawn = crate::config::config_diagram_look(config).as_str() == "handDrawn";
        // Match the writer's last requirement/element lookup, with requirement priority.
        let mut styles_by_id = BTreeMap::new();
        for node in &model.elements {
            styles_by_id.insert(node.name.as_str(), &node.css_styles);
        }
        for node in &model.requirements {
            styles_by_id.insert(node.name.as_str(), &node.css_styles);
        }
        for (id, styles) in styles_by_id {
            work.charge(
                1usize.saturating_add(
                    styles
                        .iter()
                        .fold(0usize, |total, raw| total.saturating_add(raw.len())),
                ),
            )?;
            let source = parse_node_style_overrides(styles, html);
            let source_owns_fill = source.fill.is_some();
            let source_owns_stroke = source.stroke.is_some();
            let index = self.index_for_node_id(id);
            let typed_fill = index.and_then(|index| self.typed_fill(index, source_owns_fill));
            let typed_stroke = index.and_then(|index| self.typed_stroke(index, source_owns_stroke));
            let fill = source
                .fill
                .as_deref()
                .or_else(|| typed_fill.map(|(_, css)| css))
                .unwrap_or(&self.css.default_fill)
                .to_owned();
            let stroke = source
                .stroke
                .as_deref()
                .or_else(|| typed_stroke.map(|(_, css)| css))
                .unwrap_or(&self.css.default_stroke)
                .to_owned();
            let path_style = if !hand_drawn && self.css.source_path_style_active {
                source.node_styles.as_str()
            } else {
                ""
            };
            let mut fill_style = path_style.to_owned();
            if source_owns_fill || typed_fill.is_some() {
                if !fill_style.is_empty() {
                    fill_style.push(';');
                }
                write!(fill_style, "fill:{fill} !important").unwrap();
            }
            let fill_style_attr = if fill_style.is_empty() {
                String::new()
            } else {
                format!(r#" style="{}""#, crate::svg::escape_xml(&fill_style))
            };
            let mut stroke_style = path_style.to_owned();
            if source_owns_stroke || typed_stroke.is_some() {
                if !stroke_style.is_empty() {
                    stroke_style.push(';');
                }
                write!(stroke_style, "stroke:{stroke} !important").unwrap();
            }
            let stroke_style_declaration = (!stroke_style.is_empty()).then_some(stroke_style);
            let stroke_style_attr = stroke_style_declaration
                .as_deref()
                .map(|style| format!(r#" style="{}""#, crate::svg::escape_xml(style)))
                .unwrap_or_default();
            let visual = RequirementNodeVisual {
                typed_fill: typed_fill.map(|(rule, css)| (rule, css.to_owned())),
                typed_stroke: typed_stroke.map(|(rule, css)| (rule, css.to_owned())),
                label_styles: source.label_styles,
                label_div_style_prefix: source.label_div_style_prefix,
                svg_text_styles: source.svg_text_styles,
                node_styles: source.node_styles,
                source_text_color: source.color,
                source_owns_fill,
                source_owns_stroke,
                fill,
                stroke,
                stroke_width: source.stroke_width.unwrap_or(1.3),
                fill_style_attr,
                stroke_style_declaration,
                stroke_style_attr,
            };
            let bytes = [
                &visual.label_styles,
                &visual.label_div_style_prefix,
                &visual.svg_text_styles,
                &visual.node_styles,
                &visual.fill,
                &visual.stroke,
                &visual.fill_style_attr,
                &visual.stroke_style_attr,
            ]
            .into_iter()
            .fold(0usize, |bytes, value| bytes.saturating_add(value.len()));
            work.charge(
                bytes
                    .saturating_add(
                        visual
                            .stroke_style_declaration
                            .as_deref()
                            .map_or(0, str::len),
                    )
                    .saturating_add(visual.source_text_color.as_deref().map_or(0, str::len)),
            )?;
            self.node_visuals.insert(id.to_owned(), visual);
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_visual_keeps_property_order_last_value_and_requirement_identity() {
        let model: RequirementDiagramRenderModel = serde_json::from_value(serde_json::json!({
            "requirements": [
                {"name": "same", "type": "Requirement", "cssStyles": ["fill:#000000"]},
                {"name": "same", "type": "Requirement", "cssStyles": ["fill:#112233", "stroke:#445566", "fill:#778899 !important", "color:#abcdef"]}
            ],
            "elements": [{"name": "same", "type": "component", "cssStyles": ["fill:#ffffff"]}]
        })).unwrap();
        let work = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        for html in [false, true] {
            let config = MermaidConfig::from_value(serde_json::json!({"htmlLabels": html}));
            let plan =
                RequirementPaintThemePlan::resolve_with_title(None, &config, &model, None, &work)
                    .unwrap();
            let visual = plan.node_visual("same").unwrap();
            assert_eq!(visual.fill, "#778899");
            assert_eq!(visual.stroke, "#445566");
            assert_eq!(
                visual.node_styles,
                "fill:#778899 !important;stroke:#445566 !important"
            );
            assert!(visual.source_owns_fill && visual.source_owns_stroke);
            assert!(visual.typed_fill.is_none() && visual.typed_stroke.is_none());
            assert_eq!(visual.label_styles, "color:#abcdef !important");
            assert_eq!(
                visual.svg_text_styles,
                if html { "" } else { "fill:#abcdef !important" }
            );
        }
    }

    #[test]
    fn same_value_source_paint_keeps_independent_typed_property() {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
            ThemeStylePatch,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap())
                            .with_stroke(CanvasPaint::solid("#654321").unwrap()),
                    )),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::REQUIREMENT);
        let model: RequirementDiagramRenderModel = serde_json::from_value(serde_json::json!({
            "requirements": [{"name": "a", "type": "Requirement", "cssStyles": ["fill:#123456"]}]
        }))
        .unwrap();
        let config = MermaidConfig::empty_object();
        let work = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = RequirementPaintThemePlan::resolve_with_title(
            Some(&resolved),
            &config,
            &model,
            None,
            &work,
        )
        .unwrap();
        let visual = plan.node_visual("a").unwrap();
        assert!(visual.source_owns_fill);
        assert!(visual.typed_fill.is_none());
        assert!(!visual.source_owns_stroke);
        assert_eq!(
            visual
                .typed_stroke
                .as_ref()
                .map(|(_, value)| value.as_str()),
            Some("#654321")
        );
        assert_eq!(visual.fill, "#123456");
        assert_eq!(visual.stroke, "#654321");
        assert!(
            plan.terminal_receipt.get().is_none(),
            "preparation must not prove successful emission"
        );
    }
}
