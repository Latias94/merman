use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::GitGraphDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

const COMMIT_LABEL_BACKGROUND_PATH: &str = "themeVariables.commitLabelBackground";

#[derive(Debug, Clone)]
struct StaticPaintAssignment {
    route_key: FamilyThemeMechanismKey,
    paint: DirectStaticPaint,
}

#[derive(Debug)]
pub(crate) struct GitGraphStaticPaintPlan {
    node_paint: Option<super::node_paint::GitGraphNodePaintPlan>,
    assignment: Option<StaticPaintAssignment>,
    pending_key: Option<FamilyThemeMechanismKey>,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<GitGraphStaticPaintReceipt>,
    terminal_styles: OnceLock<GitGraphTerminalStyles>,
}

#[derive(Debug)]
pub(crate) struct GitGraphTerminalStyles {
    pub(crate) text_colors_requested: bool,
    pub(crate) text_color: String,
    pub(crate) common_node_border_override: Option<String>,
    pub(crate) node_border: String,
    pub(crate) main_bkg: String,
    pub(crate) tag_label_color: String,
    pub(crate) commit_label_fill: String,
    pub(crate) commit_label_background: String,
    pub(crate) commit_line_color: String,
    pub(crate) tag_background: String,
    pub(crate) tag_border: String,
    pub(crate) state_fill: String,
    border_color_fallback: Option<String>,
    pub(crate) gradient: Option<GitGraphGradient>,
    pub(crate) branch_dasharray: &'static str,
    pub(crate) commit_background_opacity: &'static str,
    pub(crate) reverse_stroke_width: String,
    pub(crate) arrow_stroke_width: String,
}

#[derive(Debug)]
pub(crate) struct GitGraphGradient {
    pub(crate) start: String,
    pub(crate) stop: String,
    pub(crate) primary_border_source: bool,
}

impl GitGraphTerminalStyles {
    pub(crate) fn border_color<'a>(
        &'a self,
        binding: &'a crate::gitgraph::GitGraphCssBinding,
        index: usize,
    ) -> &'a str {
        match self.border_color_fallback.as_deref() {
            Some(color) => color,
            None => &binding.border_color_array[index % binding.border_color_array.len()],
        }
    }
}

impl GitGraphStaticPaintPlan {
    pub(crate) fn baseline() -> Self {
        Self {
            node_paint: None,
            assignment: None,
            pending_key: None,
            evidence: FamilyThemeEvidence::default(),
            terminal_receipt: OnceLock::new(),
            terminal_styles: OnceLock::new(),
        }
    }

    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &GitGraphDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            evidence: FamilyThemeEvidence::from_theme(theme),
            ..Self::baseline()
        };
        let Some(theme) = theme else {
            return Ok(plan);
        };
        plan.node_paint =
            super::node_paint::GitGraphNodePaintPlan::resolve(theme, effective_config, work_meter)?;

        let style = theme.style_with_work_meter(
            ThemeTarget::EdgeLabelBackground,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let winners = style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let source_owns_fill = effective_config
            .get_str("theme")
            .is_some_and(crate::gitgraph::gitgraph_theme_uses_color_gen)
            || merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                COMMIT_LABEL_BACKGROUND_PATH,
            );
        let has_visible_commit_label = layout
            .commits
            .iter()
            .any(|commit| crate::gitgraph::gitgraph_commit_label_is_visible(layout, commit));
        let assignment = (has_visible_commit_label && !source_owns_fill)
            .then(|| {
                resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::EdgeLabelBackground],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten()
            .map(|paint| StaticPaintAssignment {
                route_key: FamilyThemeMechanismKey::Rule {
                    index: paint.rule_index(),
                    target: ThemeTarget::EdgeLabelBackground,
                },
                paint,
            });

        // Facets share a rule key. Settle every winning property before recording a
        // rule outcome so an owned fill cannot hide an unsupported sibling property.
        let mut observations = BTreeMap::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            work_meter.charge(1)?;
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::EdgeLabelBackground,
                facet,
                selector,
            } = route.mechanism()
            else {
                continue;
            };
            let key = theme.family_mechanism_key(route);
            let entry = observations.entry(key).or_insert((false, None));
            if !has_visible_commit_label {
                continue;
            }
            let property = resolved_style_property_for_facet(facet);
            // Commit backgrounds have no modeled ordinal identity. An unmatched ordinal
            // is unverified unless a later static winner supersedes the same property.
            let unproved_ordinal = matches!(
                selector,
                FamilyThemeSelectorShape::Ordinal {
                    variant: None | Some(ThemeVariant::Default),
                    ..
                }
            ) && !winners.iter().any(|(winner, winner_property)| {
                *winner_property == property && *winner > rule_index
            });
            if (!winners.contains(&(rule_index, property)) && !unproved_ordinal)
                || (property == ResolvedStyleProperty::Fill && source_owns_fill)
            {
                continue;
            }
            if route.disposition() == FamilyThemeDisposition::TypedAdapter
                && property == ResolvedStyleProperty::Fill
                && assignment
                    .as_ref()
                    .is_some_and(|assignment| assignment.paint.rule_index() == rule_index)
            {
                entry.0 = true;
            } else {
                entry.1.get_or_insert(unsupported_residual_for_facet(facet));
            }
        }
        for (key, (fill, residual)) in observations {
            if let Some(reason) = residual {
                plan.evidence.mark_residual(key, reason);
            } else if fill {
                plan.pending_key = Some(key);
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        plan.assignment = assignment;

        Ok(plan)
    }

    pub(crate) fn commit_label_background_css(&self) -> Option<&str> {
        self.assignment
            .as_ref()
            .map(|assignment| assignment.paint.css())
    }

    pub(crate) fn bind_terminal_styles(
        &self,
        typography: &super::GitGraphTypographyThemePlan,
        palette: &super::GitGraphNodePalettePlan,
    ) {
        self.bind_terminal_values(
            typography.css_binding(),
            palette
                .text_paint()
                .is_requested()
                .then(|| palette.text_paint().css_colors()),
            palette.branch_stylesheet_stroke(),
        );
    }

    pub(crate) fn terminal_styles(&self) -> &GitGraphTerminalStyles {
        self.terminal_styles
            .get()
            .expect("GitGraph terminal styles prepared before SVG emission")
    }

    #[cfg(test)]
    pub(crate) fn bind_terminal_values_for_test(
        &self,
        binding: &crate::gitgraph::GitGraphCssBinding,
        text_colors: Option<[Option<&str>; 3]>,
        branch_stroke: Option<&str>,
    ) {
        self.bind_terminal_values(binding, text_colors, branch_stroke);
    }

    fn bind_terminal_values(
        &self,
        binding: &crate::gitgraph::GitGraphCssBinding,
        text_colors: Option<[Option<&str>; 3]>,
        branch_stroke: Option<&str>,
    ) {
        let values = self
            .node_paint()
            .map(|plan| plan.css_values())
            .unwrap_or([None; 6]);
        let color_gen = binding.sources.use_color_gen;
        let node_border = values[4].unwrap_or(&binding.node_border);
        let main_bkg = values[1].unwrap_or(&binding.main_bkg);
        let state_fill = if color_gen {
            main_bkg
        } else {
            values[0].unwrap_or(&binding.primary_color)
        };
        let tag_background = if color_gen {
            main_bkg
        } else {
            values[2].unwrap_or(&binding.tag_label_background)
        };
        let tag_border = if color_gen {
            node_border
        } else {
            values[5].unwrap_or(&binding.tag_label_border)
        };
        let commit_label = text_colors
            .and_then(|colors| colors[2])
            .unwrap_or(&binding.commit_label_color);
        let gradient = binding.sources.use_gradient.then(|| {
            let start = binding.gradient_start(values[3]);
            GitGraphGradient {
                start: start.to_owned(),
                stop: binding.gradient_stop(start).to_owned(),
                primary_border_source: self.node_paint().is_some()
                    && binding.gradient_start.is_none(),
            }
        });
        let styles = GitGraphTerminalStyles {
            text_colors_requested: text_colors.is_some(),
            text_color: text_colors
                .and_then(|colors| colors[0])
                .unwrap_or(binding.common.text_color())
                .to_owned(),
            common_node_border_override: values[4].map(str::to_owned),
            node_border: node_border.to_owned(),
            main_bkg: main_bkg.to_owned(),
            tag_label_color: text_colors
                .and_then(|colors| colors[1])
                .unwrap_or(&binding.tag_label_color)
                .to_owned(),
            commit_label_fill: if color_gen { node_border } else { commit_label }.to_owned(),
            commit_label_background: if color_gen {
                "transparent"
            } else {
                self.commit_label_background_css()
                    .unwrap_or(&binding.commit_label_background)
            }
            .to_owned(),
            commit_line_color: branch_stroke
                .or(binding.commit_line_color.as_deref())
                .unwrap_or(binding.common.line_color())
                .to_owned(),
            tag_background: tag_background.to_owned(),
            tag_border: tag_border.to_owned(),
            state_fill: state_fill.to_owned(),
            border_color_fallback: binding
                .border_color_array
                .is_empty()
                .then(|| node_border.to_owned()),
            gradient,
            branch_dasharray: if color_gen { "4 2" } else { "2" },
            commit_background_opacity: if color_gen { "" } else { "opacity:0.5;" },
            reverse_stroke_width: if color_gen {
                binding.stroke_width.as_str()
            } else {
                "3"
            }
            .to_owned(),
            arrow_stroke_width: if binding.use_redux_geometry {
                binding.stroke_width.as_str()
            } else {
                "8"
            }
            .to_owned(),
        };
        self.terminal_styles
            .set(styles)
            .expect("GitGraph terminal styles bound once per prepared artifact");
    }

    pub(crate) fn node_paint(&self) -> Option<&super::node_paint::GitGraphNodePaintPlan> {
        self.node_paint.as_ref()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<GitGraphStaticPaintReceipt> {
        self.assignment
            .as_ref()
            .map(|assignment| GitGraphStaticPaintReceipt {
                expected_route: assignment.route_key.clone(),
                expected_value: assignment.paint.css().into(),
                css_emitted: false,
                values_match: true,
                terminal_count: 0,
            })
    }

    pub(crate) fn record_terminal_receipt(&self, receipt: GitGraphStaticPaintReceipt) -> bool {
        self.assignment.is_some()
            && receipt.proves(self)
            && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn record_css_emission(
        &self,
        receipt: &mut Option<GitGraphStaticPaintReceipt>,
        emitted_value: Option<&str>,
    ) {
        if let Some(receipt) = receipt.as_mut() {
            receipt.record_css_emission(self, emitted_value);
        }
    }

    pub(crate) fn record_commit_label_background(
        &self,
        receipt: &mut Option<GitGraphStaticPaintReceipt>,
    ) {
        if let Some(receipt) = receipt.as_mut() {
            receipt.record_commit_label_background();
        }
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(node_paint) = self.node_paint.as_ref() {
            node_paint.finish_evidence(&mut evidence);
        }
        if let Some(key) = self.pending_key.clone() {
            match self.terminal_receipt.get() {
                Some(receipt) if receipt.proves(self) => {
                    evidence.mark_applied_with_capabilities(
                        key,
                        self.assignment
                            .as_ref()
                            .map(|assignment| [assignment.paint.capability()])
                            .into_iter()
                            .flatten(),
                    );
                }
                Some(_) => evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint),
                None => evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint),
            }
        }
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct GitGraphStaticPaintReceipt {
    expected_route: FamilyThemeMechanismKey,
    expected_value: Box<str>,
    css_emitted: bool,
    values_match: bool,
    terminal_count: usize,
}

impl GitGraphStaticPaintReceipt {
    fn record_css_emission(&mut self, plan: &GitGraphStaticPaintPlan, emitted_value: Option<&str>) {
        self.values_match &= emitted_value == plan.commit_label_background_css()
            && emitted_value == Some(self.expected_value.as_ref());
        self.css_emitted = true;
    }

    fn record_commit_label_background(&mut self) {
        self.terminal_count = self.terminal_count.saturating_add(1);
    }

    fn proves(&self, plan: &GitGraphStaticPaintPlan) -> bool {
        self.css_emitted
            && self.values_match
            && self.expected_route
                == plan
                    .assignment
                    .as_ref()
                    .map(|assignment| assignment.route_key.clone())
                    .expect("receipt requires assignment")
            && self.terminal_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::model::GitGraphCommitLayout;
    use crate::resources::RenderResourcePolicy;

    fn layout_with_label() -> GitGraphDiagramLayout {
        GitGraphDiagramLayout {
            bounds: None,
            direction: "LR".to_string(),
            rotate_commit_label: false,
            show_branches: false,
            show_commit_label: true,
            parallel_commits: false,
            diagram_padding: 0.0,
            max_pos: 0.0,
            branches: Vec::new(),
            commits: vec![GitGraphCommitLayout {
                id: "1".to_string(),
                message: "one".to_string(),
                seq: 0,
                commit_type: 0,
                parents: Vec::new(),
                branch: "main".to_string(),
                custom_type: None,
                custom_id: Some(true),
                tags: Vec::new(),
                x: 0.0,
                y: 0.0,
                pos: 0.0,
                pos_with_offset: 0.0,
            }],
            arrows: Vec::new(),
        }
    }

    #[test]
    fn prepared_default_slots_preserve_raw_gradient_and_generated_label_ownership() {
        for (theme, background, state, tag) in [
            ("base", "var(--label)", "var(--primary)", "var(--tag)"),
            ("redux-color", "transparent", "var(--main)", "var(--main)"),
        ] {
            let config = MermaidConfig::from_value(serde_json::json!({
                "theme": theme,
                "themeVariables": {
                    "useGradient": " YES ",
                    "gradientStart": "",
                    "secondaryBorderColor": "currentColor",
                    "primaryColor": "var(--primary)",
                    "mainBkg": "var(--main)",
                    "nodeBorder": "var(--border)",
                    "commitLabelBackground": "var(--label)",
                    "tagLabelBackground": "var(--tag)",
                    "lineColor": "var(--line)"
                }
            }));
            let typography = super::super::GitGraphTypographyThemePlan::resolve(None, &config);
            let palette = super::super::GitGraphNodePalettePlan::baseline(&layout_with_label());
            let plan = GitGraphStaticPaintPlan::baseline();
            plan.bind_terminal_styles(&typography, &palette);
            let styles = plan.terminal_styles();
            assert_eq!(styles.commit_label_background, background);
            assert_eq!(styles.state_fill, state);
            assert_eq!(styles.tag_background, tag);
            assert_eq!(styles.commit_line_color, "var(--line)");
            assert_eq!(
                styles.border_color(typography.css_binding(), 63),
                "var(--border)"
            );
            assert!(styles.common_node_border_override.is_none());
            let gradient = styles.gradient.as_ref().expect("coerced gradient enabled");
            assert_eq!(gradient.start, "");
            assert_eq!(gradient.stop, "currentColor");
            assert!(!gradient.primary_border_source);
        }
    }

    fn plan_with_layout(layout: &GitGraphDiagramLayout) -> GitGraphStaticPaintPlan {
        let paint = CanvasPaint::solid("#123456").expect("valid paint");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::EdgeLabelBackground,
                            ThemeStylePatch::default().with_fill(paint),
                        )
                        .for_family(crate::DiagramFamilyId::GIT_GRAPH),
                    ),
                ),
            )
            .expect("compile theme")
            .resolve(crate::DiagramFamilyId::GIT_GRAPH);
        GitGraphStaticPaintPlan::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            layout,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .expect("resolve static paint")
    }

    fn plan() -> GitGraphStaticPaintPlan {
        plan_with_layout(&layout_with_label())
    }

    #[test]
    fn receipt_requires_css_and_commit_label_background_terminal() {
        let plan = plan();
        let mut receipt = plan.begin_terminal_receipt().expect("typed receipt");
        receipt.record_css_emission(&plan, Some("#123456"));
        // The production writer records this occurrence after emitting the rect.
        receipt.record_commit_label_background();
        assert!(receipt.proves(&plan));
    }

    #[test]
    fn static_paint_is_not_applicable_without_a_visible_commit_label() {
        for (case, show_commit_label, id) in [("hidden", false, "1"), ("empty", true, "")] {
            let mut layout = layout_with_label();
            layout.show_commit_label = show_commit_label;
            layout.commits[0].id = id.to_string();

            let plan = plan_with_layout(&layout);

            assert!(
                plan.assignment.is_none(),
                "{case}: hidden or empty labels must not retain a static paint assignment"
            );
            assert!(
                plan.begin_terminal_receipt().is_none(),
                "{case}: hidden or empty labels must not create a terminal receipt"
            );
            assert_eq!(plan.commit_label_background_css(), None, "{case}");

            let evidence = plan.finish_evidence();
            assert_eq!(evidence.applied().len(), 0, "{case}");
            assert_eq!(evidence.not_applicable_mechanisms().len(), 1, "{case}");
            assert!(evidence.residuals().is_empty(), "{case}");
        }
    }
}
