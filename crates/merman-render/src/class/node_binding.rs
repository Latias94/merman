use merman_core::models::class_diagram::{ClassDiagram, ClassInterface, ClassNode};
use rustc_hash::FxHashMap;

/// Immutable terminal styles for every semantic node, including the baseline path.
#[derive(Debug)]
pub(crate) struct ClassNodeVisualPlan {
    indices: FxHashMap<String, ClassNodeVisualIndices>,
    nodes: Box<[ClassNodeVisualBinding]>,
    interfaces: Box<[ClassInterfaceVisualBinding]>,
}

#[derive(Debug)]
struct ClassNodeVisualIndices {
    node: Option<usize>,
    interface: Option<usize>,
}

impl ClassNodeVisualPlan {
    pub(crate) fn resolve(
        model: &ClassDiagram,
        relation: &super::ClassRelationThemePlan,
        typography: &super::ClassTextThemePlan,
        diagram_use_html_labels: bool,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut indices: FxHashMap<String, ClassNodeVisualIndices> = FxHashMap::default();
        let mut nodes = Vec::with_capacity(model.classes().len());
        for (id, node) in model.classes() {
            work.checkpoint(merman_core::OperationPhase::Layout)?;
            let binding = ClassNodeVisualBinding::lower(
                node.node,
                relation.node_paint(nodes.len() + 1),
                typography,
                diagram_use_html_labels,
            )?;
            indices.insert(
                id.to_string(),
                ClassNodeVisualIndices {
                    node: Some(nodes.len()),
                    interface: None,
                },
            );
            nodes.push(binding);
        }
        let mut interfaces = Vec::new();
        for (ordinal, interface) in model.interfaces().enumerate() {
            work.checkpoint(merman_core::OperationPhase::Layout)?;
            let binding = ClassInterfaceVisualBinding::lower(
                interface.interface,
                relation.node_paint(model.classes().len() + ordinal + 1),
            );
            let index = indices
                .entry(interface.id.to_string())
                .or_insert(ClassNodeVisualIndices {
                    node: None,
                    interface: None,
                });
            if let Some(slot) = index.interface {
                interfaces[slot] = binding;
            } else {
                index.interface = Some(interfaces.len());
                interfaces.push(binding);
            }
        }
        Ok(Self {
            indices,
            nodes: nodes.into_boxed_slice(),
            interfaces: interfaces.into_boxed_slice(),
        })
    }

    pub(crate) fn label_paints(&self) -> impl Iterator<Item = (&str, Option<usize>)> {
        self.indices.iter().map(|(id, index)| {
            let rule = index
                .node
                .map(|slot| self.nodes[slot].generic_text_rule)
                .or_else(|| {
                    index
                        .interface
                        .map(|slot| self.interfaces[slot].generic_text_rule)
                })
                .flatten();
            (id.as_str(), rule)
        })
    }

    pub(crate) fn node(&self, id: &str) -> &ClassNodeVisualBinding {
        let index = self
            .indices
            .get(id)
            .and_then(|indices| indices.node)
            .expect("prepared Class node visual binding");
        &self.nodes[index]
    }

    pub(crate) fn interface(&self, id: &str) -> &ClassInterfaceVisualBinding {
        let index = self
            .indices
            .get(id)
            .and_then(|indices| indices.interface)
            .expect("prepared Class interface visual binding");
        &self.interfaces[index]
    }
}

#[derive(Debug)]
pub(crate) struct ClassInterfaceVisualBinding {
    pub(crate) label_style: String,
    pub(crate) label_fill: Option<(usize, String)>,
    #[cfg(test)]
    pub(crate) source_owned: bool,
    pub(crate) emission: crate::class::ClassNodePaintOutcomes,
    generic_text_rule: Option<usize>,
}

impl ClassInterfaceVisualBinding {
    fn lower(interface: &ClassInterface, paint: &crate::class::ClassNodePaintBinding) -> Self {
        let label = crate::entities::decode_entities_minimal_cow(interface.label.trim());
        let source_owned = crate::class::class_text_is_math_only(label.as_ref());
        let label_fill = paint.typed_label_fill(source_owned);
        let label_style = class_node_label_style("", label_fill.map(|(_, css)| css));
        let verified = !crate::math::contains_delimited_math(label.as_ref());
        let emission = paint.outcomes(true, true, source_owned, !label.trim().is_empty(), verified);
        Self {
            label_style,
            label_fill: label_fill.map(|(rule, css)| (rule, css.to_owned())),
            #[cfg(test)]
            source_owned,
            emission,
            generic_text_rule: (paint.label_fill_target()
                == Some(crate::diagram_theme::ThemeTarget::Text))
            .then(|| paint.typed_label_fill(false).map(|(rule, _)| rule))
            .flatten(),
        }
    }

    pub(crate) fn emitted_label_fill(&self) -> Option<(usize, &str)> {
        self.label_fill
            .as_ref()
            .map(|(rule, css)| (*rule, css.as_str()))
    }
}

/// Paint selections and declarations resolved once before any Class node is emitted.
#[derive(Debug)]
pub(crate) struct ClassNodeVisualBinding {
    // None selects the artifact-owned default, regardless of CSS value equality.
    fill_override: Option<String>,
    stroke_override: Option<String>,
    stroke_width_override: Option<String>,
    stroke_dasharray_override: Option<String>,
    pub(crate) fill_style: String,
    pub(crate) stroke_style: String,
    pub(crate) html_label_style: String,
    pub(crate) svg_label_style: String,
    pub(crate) label_fill: Option<String>,
    pub(crate) emission: crate::class::ClassNodePaintOutcomes,
    generic_text_rule: Option<usize>,
    pub(crate) typography: crate::class::ClassTextTerminalFacts,
}

impl ClassNodeVisualBinding {
    pub(crate) fn fill<'a>(&'a self, defaults: &'a super::ClassCssThemeBinding) -> &'a str {
        self.fill_override
            .as_deref()
            .unwrap_or(&defaults.node_default_fill)
    }

    pub(crate) fn stroke<'a>(&'a self, defaults: &'a super::ClassCssThemeBinding) -> &'a str {
        self.stroke_override
            .as_deref()
            .unwrap_or(&defaults.node_default_stroke)
    }

    pub(crate) fn stroke_width(&self) -> &str {
        self.stroke_width_override.as_deref().unwrap_or("1.3")
    }

    pub(crate) fn stroke_dasharray(&self) -> &str {
        self.stroke_dasharray_override.as_deref().unwrap_or("0 0")
    }

    fn lower(
        node: &ClassNode,
        paint: &crate::class::ClassNodePaintBinding,
        typography: &crate::class::ClassTextThemePlan,
        diagram_use_html_labels: bool,
    ) -> crate::Result<Self> {
        let source = class_apply_inline_styles(node);
        let facts = if typography.requires_node_style_facts() {
            crate::class::ClassTextTerminalFacts::from_node_style_facts(
                typography.require_node_style_facts(&node.id)?,
            )
        } else {
            crate::class::ClassTextTerminalFacts::default()
        };
        let label_truth = class_node_label_terminal_truth(facts);
        let typed_fill = paint.typed_fill(source.fill.is_some());
        let typed_stroke = paint.typed_stroke(source.stroke.is_some());
        let typed_label = paint.typed_label_fill(label_truth.source_owns_paint());
        let fill_override = typed_fill.map(|(_, css)| css).or(source.fill);
        let stroke_override = typed_stroke.map(|(_, css)| css).or(source.stroke);
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
        let emission = paint.outcomes(
            source.fill.is_some(),
            source.stroke.is_some(),
            label_truth.source_owns_paint(),
            facts.visible_run_count() != 0 || !typography.requires_node_style_facts(),
            label_truth.typed_fill_verified(),
        );
        Ok(Self {
            fill_override: fill_override.map(str::to_owned),
            stroke_override: stroke_override.map(str::to_owned),
            stroke_width_override: source
                .stroke_width
                .map(|value| value.trim_end_matches("px").trim().to_owned()),
            stroke_dasharray_override: source.stroke_dasharray.map(str::to_owned),
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
            generic_text_rule: (paint.label_fill_target()
                == Some(crate::diagram_theme::ThemeTarget::Text))
            .then(|| paint.typed_label_fill(false).map(|(rule, _)| rule))
            .flatten(),
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
        let mut model = model.model().clone();
        model.interfaces.push(ClassInterface {
            id: "interface0".into(),
            label: "$$x$$".into(),
            class_id: "A".into(),
        });
        ClassDiagram::new(model, &merman_core::MermaidConfig::default())
    }

    #[test]
    fn inactive_consumers_do_not_require_a_synthetic_node_fact_map() {
        let model = fixture_model();
        let config = merman_core::presentation::PresentationConfig::default();
        let typography = super::super::ClassTextThemePlan::resolve(None, &config);
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let relation =
            super::super::ClassRelationThemePlan::resolve(None, &config, 0, 3, &meter).unwrap();
        let visual = ClassNodeVisualPlan::resolve(&model, &relation, &typography, false, &meter)
            .expect("ordinary paint has no style-fact consumer");
        let defaults = typography.css_binding();
        assert_eq!(visual.node("A").fill(defaults), defaults.node_default_fill);
        assert_eq!(
            visual.node("B").stroke(defaults),
            defaults.node_default_stroke
        );
        for binding in &visual.nodes {
            assert!(binding.fill_override.is_none());
            assert!(binding.stroke_override.is_none());
            assert!(binding.stroke_width_override.is_none());
            assert!(binding.stroke_dasharray_override.is_none());
            assert_eq!(binding.stroke_width(), "1.3");
            assert_eq!(binding.stroke_dasharray(), "0 0");
        }
        assert!(visual.interface("interface0").source_owned);
    }

    #[test]
    fn long_shared_defaults_are_borrowed_without_node_payload_growth() {
        let model = fixture_model();
        for padding in [0, 1024, 4096] {
            let fill = format!("rgb({}1,2,3)", " ".repeat(padding));
            let stroke = format!("rgb({}4,5,6)", " ".repeat(padding));
            let config =
                merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
                    "themeVariables": {"mainBkg": fill, "nodeBorder": stroke}
                }));
            let typography = crate::class::ClassTextThemePlan::resolve(None, &config);
            let defaults = typography.css_binding();
            let meter = crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let relation =
                crate::class::ClassRelationThemePlan::resolve(None, &config, 0, 3, &meter).unwrap();
            let visual =
                ClassNodeVisualPlan::resolve(&model, &relation, &typography, false, &meter)
                    .unwrap();
            assert_eq!(defaults.node_default_fill, fill);
            assert_eq!(defaults.node_default_stroke, stroke);
            for binding in &visual.nodes {
                assert!(binding.fill_override.is_none());
                assert!(binding.stroke_override.is_none());
                assert!(binding.stroke_width_override.is_none());
                assert!(binding.stroke_dasharray_override.is_none());
                assert!(std::ptr::eq(
                    binding.fill(defaults),
                    defaults.node_default_fill.as_str(),
                ));
                assert!(std::ptr::eq(
                    binding.stroke(defaults),
                    defaults.node_default_stroke.as_str(),
                ));
            }
        }
    }

    #[test]
    fn explicit_source_values_equal_to_defaults_remain_owned() {
        let mut model = fixture_model().into_model();
        let config = merman_core::presentation::PresentationConfig::default();
        let typography = crate::class::ClassTextThemePlan::resolve(None, &config);
        let defaults = typography.css_binding();
        model.classes.get_mut("A").unwrap().styles = vec![
            format!("fill:{}", defaults.node_default_fill),
            format!("stroke:{}", defaults.node_default_stroke),
        ];
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let relation =
            crate::class::ClassRelationThemePlan::resolve(None, &config, 0, 3, &meter).unwrap();
        let model = ClassDiagram::new(model, config.as_config());
        let visual =
            ClassNodeVisualPlan::resolve(&model, &relation, &typography, false, &meter).unwrap();
        let source = visual.node("A");
        let inherited = visual.node("B");
        assert_eq!(source.fill(defaults), inherited.fill(defaults));
        assert_eq!(source.stroke(defaults), inherited.stroke(defaults));
        assert_eq!(
            source.fill_override.as_deref(),
            Some(defaults.node_default_fill.as_str()),
        );
        assert_eq!(
            source.stroke_override.as_deref(),
            Some(defaults.node_default_stroke.as_str()),
        );
        assert!(inherited.fill_override.is_none());
        assert!(inherited.stroke_override.is_none());
    }

    #[test]
    fn visual_preparation_requires_sealed_layout_facts_and_observes_cancellation() {
        let model = fixture_model();
        let config = merman_core::presentation::PresentationConfig::default();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::CLASS);
        let typography = super::super::ClassTextThemePlan::resolve(Some(&theme), &config);
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
                    .classes()
                    .map(|(id, _)| (id.into(), super::super::ClassNodeLabelStyleFacts::default()))
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
            "styles": [
                "fill:var(--external) !important", "stroke-width:7px",
                "stroke-dasharray:4 2"
            ]
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
        let config = merman_core::presentation::PresentationConfig::default();
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
        let mut model = fixture_model().into_model();
        model.classes.clear();
        model.classes.insert("A".into(), node);
        model.interfaces.clear();
        let model = ClassDiagram::new(model, config.as_config());
        let prepared_work = meter.used();
        let visual =
            ClassNodeVisualPlan::resolve(&model, &plan, &typography, false, &meter).unwrap();
        let binding = visual.node("A");
        assert_eq!(binding.fill(typography.css_binding()), "var(--external)");
        assert_eq!(binding.stroke(typography.css_binding()), "#654321");
        assert_eq!(binding.stroke_width(), "7");
        assert_eq!(binding.stroke_dasharray(), "4 2");
        assert!(
            binding
                .fill_style
                .contains("fill:var(--external) !important")
        );
        assert!(!binding.fill_style.contains("#123456"));
        assert_eq!(meter.used(), prepared_work);
    }

    #[test]
    fn same_value_source_and_typed_paints_keep_distinct_authority() {
        let mut model = fixture_model().into_model();
        model.interfaces.clear();
        model.classes.get_mut("A").unwrap().styles = vec![
            "fill:#123456".into(),
            "stroke:#654321".into(),
            "stroke-width:px".into(),
            "stroke-dasharray:0 0".into(),
        ];
        model.classes.get_mut("B").unwrap().styles = vec!["stroke-width:1.3px".into()];
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
        let config = merman_core::presentation::PresentationConfig::default();
        let typography = crate::class::ClassTextThemePlan::resolve(Some(&theme), &config);
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let relation =
            crate::class::ClassRelationThemePlan::resolve(Some(&theme), &config, 0, 2, &meter)
                .unwrap();
        let model = ClassDiagram::new(model, config.as_config());
        let visual =
            ClassNodeVisualPlan::resolve(&model, &relation, &typography, false, &meter).unwrap();
        let source = visual.node("A");
        let typed = visual.node("B");
        let defaults = typography.css_binding();
        assert_eq!(source.fill(defaults), typed.fill(defaults));
        assert_eq!(source.stroke(defaults), typed.stroke(defaults));
        for binding in [source, typed] {
            assert_eq!(binding.fill_override.as_deref(), Some("#123456"));
            assert_eq!(binding.stroke_override.as_deref(), Some("#654321"));
        }
        assert!(!source.fill_style.contains("!important"));
        assert!(!source.stroke_style.contains("!important"));
        assert!(typed.fill_style.contains("fill:#123456 !important"));
        assert!(typed.stroke_style.contains("stroke:#654321 !important"));
        assert_eq!(source.stroke_width_override.as_deref(), Some(""));
        assert_eq!(source.stroke_width(), "");
        assert_eq!(source.stroke_dasharray_override.as_deref(), Some("0 0"));
        assert_eq!(typed.stroke_width_override.as_deref(), Some("1.3"));
        assert_eq!(typed.stroke_width(), "1.3");
        assert!(typed.stroke_dasharray_override.is_none());
        let mut outcomes = relation.begin_outcomes(Vec::new());
        outcomes.record_node(source.emission);
        outcomes.record_node(typed.emission);
        let recorder =
            crate::class::ClassThemeEvidenceRecorder::new(Some(&theme), 0, 0, 2, 0, Vec::new());
        recorder.record_outcomes(outcomes);
    }
}
