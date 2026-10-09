use super::ClassSvgNode;
use super::label::{
    class_apply_inline_styles, class_node_label_style, class_node_label_terminal_truth,
    class_node_paint_style, class_source_label_style,
};

pub(super) struct ClassInterfaceVisualBinding {
    pub(super) label_style: String,
    pub(super) label_fill: Option<(usize, String)>,
    pub(super) source_owned: bool,
    pub(super) verified: bool,
}

impl ClassInterfaceVisualBinding {
    pub(super) fn lower(
        interface: &super::ClassSvgInterface,
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

    pub(super) fn emitted_label_fill(&self) -> Option<(usize, &str)> {
        self.label_fill
            .as_ref()
            .map(|(rule, css)| (*rule, css.as_str()))
    }
}

/// Concrete paint and declarations selected once before any Class node is emitted.
pub(super) struct ClassNodeVisualBinding {
    pub(super) fill: String,
    pub(super) stroke: String,
    pub(super) stroke_width: String,
    pub(super) stroke_dasharray: String,
    pub(super) fill_style: String,
    pub(super) stroke_style: String,
    pub(super) html_label_style: String,
    pub(super) svg_label_style: String,
    pub(super) label_fill: Option<String>,
    pub(super) emission: crate::class::ClassNodeTerminalEmission,
    pub(super) typography: crate::class::ClassTextTerminalFacts,
}

impl ClassNodeVisualBinding {
    pub(super) fn lower(
        node: &ClassSvgNode,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };

    #[test]
    fn unknown_source_css_shadows_typed_paint_without_reinterpreting_or_recharging() {
        let node: ClassSvgNode = serde_json::from_value(serde_json::json!({
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
        let expectations = plan.resolve_node_expectations(["A".into()]);
        let prepared_work = meter.used();
        let binding =
            ClassNodeVisualBinding::lower(&node, &expectations[0], &typography, false).unwrap();
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
