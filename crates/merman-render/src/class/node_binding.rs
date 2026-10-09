use merman_core::models::class_diagram::{ClassDiagram, ClassInterface, ClassNode};
use rustc_hash::FxHashMap;

/// Immutable terminal styles for every semantic node, including the baseline path.
#[derive(Debug)]
pub(crate) struct ClassNodeVisualPlan {
    expectations: Box<[super::ClassNodeTerminalExpectation]>,
    nodes: FxHashMap<String, ClassNodeVisualBinding>,
    interfaces: FxHashMap<String, ClassInterfaceVisualBinding>,
}

impl ClassNodeVisualPlan {
    pub(crate) fn resolve(
        model: &ClassDiagram,
        relation: &super::ClassRelationThemePlan,
        typography: &super::ClassTextThemePlan,
        diagram_use_html_labels: bool,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let expectations = relation.resolve_node_expectations(
            model
                .classes
                .keys()
                .cloned()
                .chain(model.interfaces.iter().map(|node| node.id.clone())),
        );
        let by_id = expectations
            .iter()
            .map(|expectation| (expectation.id(), expectation))
            .collect::<FxHashMap<_, _>>();
        let mut nodes = FxHashMap::default();
        for (id, node) in &model.classes {
            work.checkpoint(merman_core::OperationPhase::Layout)?;
            let expectation = by_id
                .get(id.as_str())
                .expect("prepared Class node theme owner");
            nodes.insert(
                id.clone(),
                ClassNodeVisualBinding::lower(
                    node,
                    expectation,
                    typography,
                    diagram_use_html_labels,
                )?,
            );
        }
        let mut interfaces = FxHashMap::default();
        for interface in &model.interfaces {
            work.checkpoint(merman_core::OperationPhase::Layout)?;
            let expectation = by_id
                .get(interface.id.as_str())
                .expect("prepared Class interface theme owner");
            interfaces.insert(
                interface.id.clone(),
                ClassInterfaceVisualBinding::lower(interface, expectation),
            );
        }
        Ok(Self {
            expectations: expectations.into_boxed_slice(),
            nodes,
            interfaces,
        })
    }

    pub(crate) fn expectations(&self) -> &[super::ClassNodeTerminalExpectation] {
        &self.expectations
    }

    pub(crate) fn node(&self, id: &str) -> &ClassNodeVisualBinding {
        self.nodes
            .get(id)
            .expect("prepared Class node visual binding")
    }

    pub(crate) fn interface(&self, id: &str) -> &ClassInterfaceVisualBinding {
        self.interfaces
            .get(id)
            .expect("prepared Class interface visual binding")
    }
}

#[derive(Debug)]
pub(crate) struct ClassInterfaceVisualBinding {
    pub(crate) label_style: String,
    pub(crate) label_fill: Option<(usize, String)>,
    pub(crate) source_owned: bool,
    pub(crate) verified: bool,
}

impl ClassInterfaceVisualBinding {
    fn lower(
        interface: &ClassInterface,
        expectation: &crate::class::ClassNodeTerminalExpectation,
    ) -> Self {
        let label = crate::entities::decode_entities_minimal_cow(interface.label.trim());
        let source_owned = crate::class::class_text_is_math_only(label.as_ref());
        let label_fill = expectation.typed_label_fill(source_owned);
        Self {
            label_style: class_node_label_style("", label_fill.map(|(_, css)| css)),
            label_fill: label_fill.map(|(rule, css)| (rule, css.to_owned())),
            source_owned,
            verified: !crate::math::contains_delimited_math(label.as_ref()),
        }
    }

    pub(crate) fn emitted_label_fill(&self) -> Option<(usize, &str)> {
        self.label_fill
            .as_ref()
            .map(|(rule, css)| (*rule, css.as_str()))
    }
}

/// Concrete paint and declarations selected once before any Class node is emitted.
#[derive(Debug)]
pub(crate) struct ClassNodeVisualBinding {
    pub(crate) fill: String,
    pub(crate) stroke: String,
    pub(crate) stroke_width: String,
    pub(crate) stroke_dasharray: String,
    pub(crate) fill_style: String,
    pub(crate) stroke_style: String,
    pub(crate) html_label_style: String,
    pub(crate) svg_label_style: String,
    pub(crate) label_fill: Option<String>,
    pub(crate) emission: crate::class::ClassNodeTerminalEmission,
    pub(crate) typography: crate::class::ClassTextTerminalFacts,
}

impl ClassNodeVisualBinding {
    fn lower(
        node: &ClassNode,
        expectation: &crate::class::ClassNodeTerminalExpectation,
        typography: &crate::class::ClassTextThemePlan,
        diagram_use_html_labels: bool,
    ) -> crate::Result<Self> {
        let source = class_apply_inline_styles(node);
        let facts = crate::class::ClassTextTerminalFacts::from_node_style_facts(
            typography.require_node_style_facts(&node.id)?,
        );
        let label_truth = class_node_label_terminal_truth(facts);
        let typed_fill = expectation.typed_fill(source.fill.is_some());
        let typed_stroke = expectation.typed_stroke(source.stroke.is_some());
        let typed_label = expectation.typed_label_fill(label_truth.source_owns_paint());
        let defaults = typography.css_binding();
        let fill = typed_fill
            .map(|(_, css)| css)
            .or(source.fill)
            .unwrap_or(&defaults.node_default_fill)
            .to_owned();
        let stroke = typed_stroke
            .map(|(_, css)| css)
            .or(source.stroke)
            .unwrap_or(&defaults.node_default_stroke)
            .to_owned();
        let html_label_style =
            class_node_label_style(&source.style_attr, typed_label.map(|(_, css)| css));
        let svg_label_style = class_node_label_style(
            &class_source_label_style(source.color),
            typed_label.map(|(_, css)| css),
        );
        let label_style = if diagram_use_html_labels || crate::class::class_node_requires_math(node)
        {
            &html_label_style
        } else {
            &svg_label_style
        };
        let emission = crate::class::ClassNodeTerminalEmission::new(
            &node.id,
            crate::class::ClassNodePaintTerminalEmission::new(
                source.fill.is_some(),
                typed_fill,
                &fill,
            )
            .with_source_value(source.fill),
            crate::class::ClassNodePaintTerminalEmission::new(
                source.stroke.is_some(),
                typed_stroke,
                &stroke,
            )
            .with_source_value(source.stroke),
            crate::class::ClassNodePaintTerminalEmission::new(
                label_truth.source_owns_paint(),
                typed_label,
                label_style,
            )
            .with_source_value(source.color)
            .with_terminal_verified(label_truth.typed_fill_verified()),
        );
        Ok(Self {
            fill,
            stroke,
            stroke_width: source
                .stroke_width
                .unwrap_or("1.3")
                .trim_end_matches("px")
                .trim()
                .to_owned(),
            stroke_dasharray: source.stroke_dasharray.unwrap_or("0 0").to_owned(),
            fill_style: class_node_paint_style(
                &source.style_attr,
                "fill",
                typed_fill.map(|(_, css)| css),
            ),
            stroke_style: class_node_paint_style(
                &source.style_attr,
                "stroke",
                typed_stroke.map(|(_, css)| css),
            ),
            label_fill: typed_label.map(|(_, css)| css.to_owned()),
            typography: facts.with_paint(typed_label, label_style),
            html_label_style,
            svg_label_style,
            emission,
        })
    }
}

struct ClassInlineStyles<'a> {
    pub style_attr: String,
    pub color: Option<&'a str>,
    pub fill: Option<&'a str>,
    pub stroke: Option<&'a str>,
    pub stroke_width: Option<&'a str>,
    pub stroke_dasharray: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
struct ClassNodeLabelTerminalTruth {
    source_owns_paint: bool,
    typed_fill_verified: bool,
}

impl ClassNodeLabelTerminalTruth {
    const fn source_owns_paint(self) -> bool {
        self.source_owns_paint
    }

    const fn typed_fill_verified(self) -> bool {
        self.typed_fill_verified
    }
}

fn class_apply_inline_styles<'a>(node: &'a ClassNode) -> ClassInlineStyles<'a> {
    let mut style_attr = String::new();
    let mut color: Option<&str> = None;
    let mut fill: Option<&str> = None;
    let mut stroke: Option<&str> = None;
    let mut stroke_width: Option<&str> = None;
    let mut stroke_dasharray: Option<&str> = None;

    for raw in &node.styles {
        let Some(parsed) = crate::mermaid_style::parse_style_declaration(raw) else {
            continue;
        };
        if !style_attr.is_empty() {
            style_attr.push(';');
        }
        style_attr.push_str(parsed.property_css());
        style_attr.push(':');
        style_attr.push_str(parsed.source_value());

        match parsed.property() {
            "color" => color = Some(parsed.value()),
            "fill" => fill = Some(parsed.value()),
            "stroke" => stroke = Some(parsed.value()),
            "stroke-width" => stroke_width = Some(parsed.value()),
            "stroke-dasharray" => stroke_dasharray = Some(parsed.value()),
            _ => {}
        }
    }

    ClassInlineStyles {
        style_attr,
        color,
        fill,
        stroke,
        stroke_width,
        stroke_dasharray,
    }
}

fn class_node_label_terminal_truth(
    terminal_facts: crate::class::ClassTextTerminalFacts,
) -> ClassNodeLabelTerminalTruth {
    ClassNodeLabelTerminalTruth {
        source_owns_paint: terminal_facts.source_owns_every_visible_run(),
        typed_fill_verified: terminal_facts.paint_ownership_is_unambiguous(),
    }
}

fn class_source_label_style(color: Option<&str>) -> String {
    color.map_or_else(String::new, |color| format!("color:{color};fill:{color}"))
}

pub(crate) fn class_node_label_style(source_style: &str, typed_fill: Option<&str>) -> String {
    let mut style = source_style.trim().trim_end_matches(';').to_string();
    let Some(fill) = typed_fill else {
        return style;
    };
    if !style.is_empty() {
        style.push(';');
    }
    style.push_str("color:");
    style.push_str(fill);
    style.push_str(" !important;fill:");
    style.push_str(fill);
    style.push_str(" !important");
    style
}

pub(crate) fn class_node_paint_style(
    source_style: &str,
    property: &str,
    typed_paint: Option<&str>,
) -> String {
    let mut style = source_style.trim().trim_end_matches(';').to_string();
    let Some(paint) = typed_paint else {
        return style;
    };
    if !style.is_empty() {
        style.push(';');
    }
    style.push_str(property);
    style.push(':');
    style.push_str(paint);
    style.push_str(" !important");
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };

    fn fixture_model() -> ClassDiagram {
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nclass A\nclass B\n",
                merman_core::ParseOptions::default(),
            )
            .unwrap()
            .unwrap();
        let merman_core::RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class model");
        };
        let mut model = model.clone();
        model.interfaces.push(ClassInterface {
            id: "interface0".into(),
            label: "$$x$$".into(),
            class_id: "A".into(),
        });
        model
    }

    #[test]
    fn baseline_visual_plan_is_complete_ordered_and_reusable_before_emission() {
        let model = fixture_model();
        let config = merman_core::MermaidConfig::default();
        let typography = super::super::ClassTextThemePlan::resolve(None, &config);
        assert!(
            typography.seal_node_style_facts(
                model
                    .classes
                    .keys()
                    .map(|id| (
                        id.clone().into_boxed_str(),
                        super::super::ClassNodeLabelStyleFacts::default(),
                    ))
                    .collect()
            )
        );
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let relation =
            super::super::ClassRelationThemePlan::resolve(None, &config, 0, 3, &meter).unwrap();
        let used = meter.used();
        let visual =
            ClassNodeVisualPlan::resolve(&model, &relation, &typography, false, &meter).unwrap();
        assert_eq!(
            visual
                .expectations()
                .iter()
                .map(|item| item.id())
                .collect::<Vec<_>>(),
            ["A", "B", "interface0"]
        );
        assert_eq!(
            visual.node("A").fill,
            typography.css_binding().node_default_fill
        );
        assert_eq!(
            visual.node("B").stroke,
            typography.css_binding().node_default_stroke
        );
        assert!(visual.interface("interface0").source_owned);
        assert!(!visual.interface("interface0").verified);
        assert!(
            visual
                .interface("interface0")
                .emitted_label_fill()
                .is_none()
        );
        let first = visual.node("A");
        let _event = first.emission.clone();
        assert!(std::ptr::eq(first, visual.node("A")));
        assert_eq!(meter.used(), used);
    }

    #[test]
    fn visual_preparation_requires_sealed_layout_facts_and_observes_cancellation() {
        let model = fixture_model();
        let config = merman_core::MermaidConfig::default();
        let typography = super::super::ClassTextThemePlan::resolve(None, &config);
        let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input();
        let meter = crate::resources::OperationWorkMeter::new(policy);
        let relation =
            super::super::ClassRelationThemePlan::resolve(None, &config, 0, 3, &meter).unwrap();
        assert!(
            ClassNodeVisualPlan::resolve(&model, &relation, &typography, false, &meter).is_err()
        );
        assert!(
            typography.seal_node_style_facts(
                model
                    .classes
                    .keys()
                    .map(|id| (
                        id.clone().into_boxed_str(),
                        super::super::ClassNodeLabelStyleFacts::default(),
                    ))
                    .collect(),
            )
        );
        let control = merman_core::OperationControl::new();
        control.cancel_after_checkpoints(1);
        let cancelled = crate::resources::OperationWorkMeter::new_with_control(policy, control);
        let error = ClassNodeVisualPlan::resolve(&model, &relation, &typography, false, &cancelled)
            .unwrap_err();
        assert!(matches!(error, crate::Error::Cancelled(_)));
    }

    #[test]
    fn unknown_source_css_shadows_typed_paint_without_reinterpreting_or_recharging() {
        let node: ClassNode = serde_json::from_value(serde_json::json!({
            "id": "A", "label": "A", "text": "A", "domId": "A",
            "styles": ["fill:var(--external) !important", "stroke-width:7px"]
        }))
        .unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap())
                            .with_stroke(CanvasPaint::solid("#654321").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::CLASS);
        let config = merman_core::MermaidConfig::default();
        let typography = crate::class::ClassTextThemePlan::resolve(Some(&theme), &config);
        assert!(
            typography.seal_node_style_facts(std::collections::BTreeMap::from([(
                "A".into(),
                crate::class::ClassNodeLabelStyleFacts::default()
            )]))
        );
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan =
            crate::class::ClassRelationThemePlan::resolve(Some(&theme), &config, 0, 1, &meter)
                .unwrap();
        let mut model = fixture_model();
        model.classes.clear();
        model.classes.insert("A".into(), node);
        model.interfaces.clear();
        let prepared_work = meter.used();
        let visual =
            ClassNodeVisualPlan::resolve(&model, &plan, &typography, false, &meter).unwrap();
        let binding = visual.node("A");
        assert_eq!(binding.fill, "var(--external)");
        assert_eq!(binding.stroke, "#654321");
        assert_eq!(binding.stroke_width, "7");
        assert!(
            binding
                .fill_style
                .contains("fill:var(--external) !important")
        );
        assert!(!binding.fill_style.contains("#123456"));
        assert_eq!(meter.used(), prepared_work);
    }
}
