use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::diagrams::tree_view::{
    TreeViewDiagramRenderModel, TreeViewNodeRenderModel as TreeViewNode,
};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, Specified,
    ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::config::DEFAULT_LINE_THICKNESS;

/// Final Tree View theme shared by layout, terminal SVG emission, and family evidence.
#[derive(Debug)]
pub(crate) struct TreeViewThemePlan {
    line_thickness_override: Option<TreeViewTerminalStrokeWidth>,
    expected_line_count: usize,
    has_visible_typography: bool,
    label_color: Option<TreeViewPaintAssignment>,
    line_color: Option<TreeViewPaintAssignment>,
    icon_color: Option<TreeViewPaintAssignment>,
    icon_fallback_color: Option<Box<str>>,
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    pending_stroke_width_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<TreeViewThemeReceipt>,
}

#[derive(Debug, Clone)]
struct TreeViewPaintAssignment {
    key: FamilyThemeMechanismKey,
    paint: DirectStaticPaint,
}

#[derive(Debug, Clone, Copy)]
enum TreeViewPaintProperty {
    Fill,
    StrokeThenFill,
}

fn resolve_tree_view_paint(
    theme: &ResolvedDiagramTheme,
    work_meter: &OperationWorkMeter,
    targets: &[ThemeTarget],
    property: TreeViewPaintProperty,
    source_owned: bool,
    residuals: &mut BTreeMap<FamilyThemeMechanismKey, FamilyThemeResidualReason>,
) -> Result<Option<TreeViewPaintAssignment>, OperationWorkError> {
    for target in targets.iter().copied() {
        let style = if target == ThemeTarget::NodeLabel {
            // Tree View labels inherit the generic Text target before a more specific
            // NodeLabel rule wins. Keep that cascade in the resolved style rather than
            // rebuilding it in this family adapter.
            theme.text_style_with_work_meter(target, ThemeVariant::Default, None, work_meter)?
        } else {
            theme.style_with_work_meter(target, ThemeVariant::Default, None, work_meter)?
        };
        let properties: &[ResolvedStyleProperty] = match property {
            TreeViewPaintProperty::Fill => &[ResolvedStyleProperty::Fill],
            TreeViewPaintProperty::StrokeThenFill => {
                &[ResolvedStyleProperty::Stroke, ResolvedStyleProperty::Fill]
            }
        };
        for selected_property in properties {
            let resolution = match selected_property {
                ResolvedStyleProperty::Fill => style.fill_resolution(),
                ResolvedStyleProperty::Stroke => style.stroke_resolution(),
                _ => unreachable!("Tree View paint resolver only selects fill/stroke"),
            };
            let Some(origin) = resolution.winner() else {
                continue;
            };
            let facet = match selected_property {
                ResolvedStyleProperty::Fill => FamilyThemeRuleFacet::fill(resolution.specified()),
                ResolvedStyleProperty::Stroke => {
                    FamilyThemeRuleFacet::stroke(resolution.specified())
                }
                _ => None,
            };
            let Some(facet) = facet else {
                residuals.insert(
                    FamilyThemeMechanismKey::Rule {
                        index: origin.rule_index(),
                        target: origin.target(),
                    },
                    FamilyThemeResidualReason::UnsupportedPaint,
                );
                return Ok(None);
            };
            let key = FamilyThemeMechanismKey::Rule {
                index: origin.rule_index(),
                target: origin.target(),
            };
            if source_owned {
                return Ok(None);
            }
            if theme.rule_facet_disposition(origin.rule_index(), facet)
                != Some(FamilyThemeDisposition::TypedAdapter)
            {
                if theme.rule_facet_disposition(origin.rule_index(), facet)
                    == Some(FamilyThemeDisposition::Unsupported)
                {
                    residuals.insert(key, unsupported_residual_for_facet(facet));
                }
                return Ok(None);
            }
            let paint = match selected_property {
                ResolvedStyleProperty::Fill => resolve_direct_static_fill(
                    theme,
                    &style,
                    targets,
                    DirectStaticSelectorDomain::Default,
                ),
                ResolvedStyleProperty::Stroke => resolve_direct_static_stroke(
                    theme,
                    &style,
                    targets,
                    DirectStaticSelectorDomain::Default,
                ),
                _ => None,
            };
            let Some(paint) = paint else {
                residuals.insert(key, FamilyThemeResidualReason::UnsupportedPaint);
                return Ok(None);
            };
            return Ok(Some(TreeViewPaintAssignment { key, paint }));
        }
    }
    Ok(None)
}

#[derive(Debug)]
struct TreeViewTerminalStrokeWidth {
    value_px: f64,
    token: Box<str>,
}

impl TreeViewThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        model: &TreeViewDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline(0));
        };
        let (node_count, expected_line_count) = tree_view_terminal_counts(&model.root);
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(Some(theme), effective_config);
        let label_color;
        let line_color;
        let icon_color;
        let mut paint_residuals =
            BTreeMap::<FamilyThemeMechanismKey, FamilyThemeResidualReason>::new();

        let mermaid_owns_line_thickness =
            merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "treeView.lineThickness",
            );
        let static_style = theme.style_with_work_meter(
            ThemeTarget::Edge,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_winners = static_style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let static_line_thickness_candidate = (!mermaid_owns_line_thickness)
            .then(|| typed_line_thickness(theme, &static_style))
            .flatten();
        let static_winner_rule = static_line_thickness_candidate.as_ref().and_then(|_| {
            static_style
                .stroke_width_resolution()
                .winner()
                .map(|origin| origin.rule_index())
        });
        let direct_static_winner_rule = static_winner_rule
            .filter(|rule_index| has_direct_static_stroke_width_route(theme, *rule_index));

        let has_ordinal_edge_rules = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Edge && rule.ordinal().is_some());
        let mut occurrence_winners = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut static_width_wins_every_line =
            expected_line_count != 0 && direct_static_winner_rule.is_some();
        if expected_line_count != 0 {
            if has_ordinal_edge_rules {
                for ordinal in 1..=expected_line_count {
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Edge,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    occurrence_winners.extend(
                        style
                            .winner_rule_properties()
                            .into_iter()
                            .map(|(property, origin)| (origin.rule_index(), property)),
                    );
                    static_width_wins_every_line &= style
                        .stroke_width_resolution()
                        .winner()
                        .map(|origin| origin.rule_index())
                        == direct_static_winner_rule;
                }
            } else {
                occurrence_winners.extend(static_winners.iter().copied());
            }
        }
        let terminal_winner_rule = static_width_wins_every_line
            .then_some(direct_static_winner_rule)
            .flatten();
        let line_thickness_override = terminal_winner_rule.and(static_line_thickness_candidate);

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));

        let label_config_owned = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.treeView.labelColor",
        );
        let line_config_owned = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.treeView.lineColor",
        );
        let icon_config_owned = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.treeView.iconColor",
        );
        let icon_compatibility_owned = merman_core::__private::fallback_overlay_owns_path(
            effective_config,
            "themeVariables.treeView.iconColor",
        );
        let icon_terminal_owned = icon_config_owned || icon_compatibility_owned;

        // Text is the broad fallback; a NodeLabel winner is the terminal-specific winner.
        label_color = resolve_tree_view_paint(
            theme,
            work_meter,
            &[ThemeTarget::NodeLabel, ThemeTarget::Text],
            TreeViewPaintProperty::Fill,
            label_config_owned,
            &mut paint_residuals,
        )?;
        // Mermaid's line terminal is stroke-first. A fill-only Edge rule is the documented
        // fallback for themes that do not provide a stroke assignment.
        line_color = resolve_tree_view_paint(
            theme,
            work_meter,
            &[ThemeTarget::Edge],
            TreeViewPaintProperty::StrokeThenFill,
            line_config_owned,
            &mut paint_residuals,
        )?;
        // Marker is explicit when present; otherwise icon paint inherits the resolved Edge line.
        icon_color = resolve_tree_view_paint(
            theme,
            work_meter,
            &[ThemeTarget::Marker],
            TreeViewPaintProperty::StrokeThenFill,
            icon_terminal_owned,
            &mut paint_residuals,
        )?;
        let icon_fallback_color = (!icon_terminal_owned)
            .then(|| {
                line_color
                    .as_ref()
                    .map(|assignment| assignment.paint.css().into())
            })
            .flatten();

        for (key, reason) in &paint_residuals {
            evidence.mark_residual(key.clone(), *reason);
        }
        let mut observations = BTreeMap::<usize, TreeViewEdgeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Edge,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    if !selector.ordinal_domain_intersects_occurrence_count(expected_line_count) {
                        continue;
                    }
                    let route_won = occurrence_winners.contains(&(rule_index, property));
                    if !route_won
                        || (mermaid_owns_line_thickness
                            && facet == FamilyThemeRuleFacet::StrokeWidth)
                    {
                        continue;
                    }

                    observation.applicable = true;
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::StrokeWidth,
                        ) if terminal_winner_rule == Some(rule_index) => {
                            observation.stroke_width_pending = true;
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let mut pending_stroke_width_key = None;
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Edge,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed rule stays unaccounted until every winning facet has a terminal owner.
            } else if observation.stroke_width_pending {
                debug_assert!(pending_stroke_width_key.is_none());
                pending_stroke_width_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        let absent = TerminalVariantDomain::uniform(0, ThemeVariant::Default);
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Node,
                    TerminalVariantDomain::uniform(node_count, ThemeVariant::Default),
                ),
                UnsupportedTerminalDomain::direct(ThemeTarget::Title, absent),
                UnsupportedTerminalDomain::direct(ThemeTarget::EdgeLabelBackground, absent),
                UnsupportedTerminalDomain::direct(ThemeTarget::Cluster, absent),
                UnsupportedTerminalDomain::direct(ThemeTarget::ClusterLabel, absent),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Edge,
                    TerminalVariantDomain::uniform(expected_line_count, ThemeVariant::Default),
                ),
            ],
            work_meter,
        )?;

        let selected_paint_keys = [
            label_color.as_ref(),
            line_color.as_ref(),
            icon_color.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(|assignment| assignment.key.clone())
        .collect::<BTreeSet<_>>();
        let pending_stroke_width_key = pending_stroke_width_key.clone();
        for route in theme.family_mechanism_routes().iter().copied() {
            let key = theme.family_mechanism_key(route);
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(_) => {}
                FamilyThemeMechanism::RuleFacet {
                    target:
                        ThemeTarget::NodeLabel
                        | ThemeTarget::Text
                        | ThemeTarget::Edge
                        | ThemeTarget::Marker,
                    facet: FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_),
                    ..
                } if route.disposition() == FamilyThemeDisposition::TypedAdapter => {
                    if !selected_paint_keys.contains(&key) && !paint_residuals.contains_key(&key) {
                        evidence.mark_not_applicable(key);
                    }
                }
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Edge,
                    facet: FamilyThemeRuleFacet::StrokeWidth,
                    ..
                } if route.disposition() == FamilyThemeDisposition::TypedAdapter => {
                    if pending_stroke_width_key.as_ref() != Some(&key) {
                        evidence.mark_not_applicable(key);
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            line_thickness_override,
            expected_line_count,
            has_visible_typography: node_count != 0,
            label_color,
            line_color,
            icon_color,
            icon_fallback_color,
            inherited_font_stack,
            evidence,
            pending_stroke_width_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(expected_line_count: usize) -> Self {
        Self {
            line_thickness_override: None,
            expected_line_count,
            has_visible_typography: false,
            label_color: None,
            line_color: None,
            icon_color: None,
            icon_fallback_color: None,
            inherited_font_stack: InheritedFontStackPlan::resolve_property_local(
                None,
                &merman_core::MermaidConfig::default(),
            ),
            evidence: FamilyThemeEvidence::default(),
            pending_stroke_width_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn line_thickness_px(&self, mermaid_line_thickness: f64) -> f64 {
        self.line_thickness_override
            .as_ref()
            .map_or(mermaid_line_thickness, |width| width.value_px)
    }

    pub(crate) fn terminal_stroke_width_token(&self, emitted_stroke_width_px: f64) -> Option<&str> {
        self.line_thickness_override
            .as_ref()
            .filter(|width| width.value_px.to_bits() == emitted_stroke_width_px.to_bits())
            .map(|width| width.token.as_ref())
    }

    pub(crate) fn additional_paint_outset_px(&self) -> f64 {
        self.line_thickness_override.as_ref().map_or(0.0, |width| {
            ((width.value_px - DEFAULT_LINE_THICKNESS).max(0.0)) / 2.0
        })
    }

    pub(crate) fn record_terminal(&self, receipt: TreeViewThemeReceipt) -> bool {
        receipt.proves(self) && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, self.has_visible_typography);
        if let Some(receipt) = self.terminal_receipt.get() {
            if let Some(key) = self.pending_stroke_width_key.clone() {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::BorderStyling]);
            }
            if let Some(assignment) = &self.label_color {
                if receipt.label_color_matches(assignment.paint.css()) {
                    evidence.mark_applied_with_capabilities(
                        assignment.key.clone(),
                        [assignment.paint.capability(), ThemeCapability::SolidPaint],
                    );
                }
            }
            if let Some(assignment) = &self.line_color {
                if receipt.line_color_matches(assignment.paint.css()) {
                    evidence.mark_applied_with_capabilities(
                        assignment.key.clone(),
                        [
                            assignment.paint.capability(),
                            ThemeCapability::BorderStyling,
                        ],
                    );
                }
            }
            if let Some(assignment) = &self.icon_color {
                if receipt.expected_icon_count == 0 {
                    evidence.mark_not_applicable(assignment.key.clone());
                } else if receipt.icon_color_matches(assignment.paint.css()) {
                    evidence.mark_applied_with_capabilities(
                        assignment.key.clone(),
                        [assignment.paint.capability(), ThemeCapability::SolidPaint],
                    );
                }
            }
        }
        if self.inherited_font_stack.typed_font_stack_requested() {
            let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
            match self.inherited_font_stack.outcome() {
                InheritedFontStackOutcome::Typed if !self.has_visible_typography => {
                    evidence.mark_not_applicable(key);
                }
                InheritedFontStackOutcome::Typed
                    if self.terminal_receipt.get().is_some_and(|receipt| {
                        receipt.font_family_matches(self.inherited_font_stack.font_family_css())
                    }) =>
                {
                    evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                }
                InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                InheritedFontStackOutcome::Typed | InheritedFontStackOutcome::Unsupported => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                }
                InheritedFontStackOutcome::Inactive => {}
            }
        }
        evidence
    }

    pub(crate) fn label_color_css<'a>(&'a self, baseline: &'a str) -> &'a str {
        self.label_color
            .as_ref()
            .map_or(baseline, |assignment| assignment.paint.css())
    }

    pub(crate) fn line_color_css<'a>(&'a self, baseline: &'a str) -> &'a str {
        self.line_color
            .as_ref()
            .map_or(baseline, |assignment| assignment.paint.css())
    }

    pub(crate) fn icon_color_css<'a>(&'a self, baseline: &'a str) -> &'a str {
        self.icon_color
            .as_ref()
            .map(|assignment| assignment.paint.css())
            .or_else(|| self.icon_fallback_color.as_deref())
            .unwrap_or(baseline)
    }

    pub(crate) fn font_family_css<'a>(&'a self, baseline: &'a str) -> &'a str {
        if self.inherited_font_stack.typed_font_stack_requested() {
            self.inherited_font_stack.font_family_css()
        } else {
            baseline
        }
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        label_count: usize,
        icon_count: usize,
    ) -> TreeViewThemeReceipt {
        TreeViewThemeReceipt::new(
            self.label_color
                .as_ref()
                .map(|assignment| assignment.paint.css()),
            self.line_color
                .as_ref()
                .map(|assignment| assignment.paint.css()),
            (self
                .icon_color
                .as_ref()
                .map(|assignment| assignment.paint.css()))
            .or_else(|| self.icon_fallback_color.as_deref()),
            self.inherited_font_stack.font_family_css(),
            label_count,
            icon_count,
            self.expected_line_count,
            self.line_thickness_override
                .as_ref()
                .map(|width| width.token.as_ref()),
        )
    }
}

fn typed_line_thickness(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<TreeViewTerminalStrokeWidth> {
    let origin = style.stroke_width_resolution().winner()?;
    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::StrokeWidth)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    match style.stroke_width_resolution().specified() {
        Specified::Value(value) => Some(TreeViewTerminalStrokeWidth {
            value_px: f64::from(*value),
            token: value.to_string().into_boxed_str(),
        }),
        Specified::Clear => Some(TreeViewTerminalStrokeWidth {
            value_px: DEFAULT_LINE_THICKNESS,
            token: DEFAULT_LINE_THICKNESS.to_string().into_boxed_str(),
        }),
        Specified::Unspecified => None,
    }
}

fn has_direct_static_stroke_width_route(
    theme: &ResolvedDiagramTheme,
    expected_rule_index: usize,
) -> bool {
    theme
        .family_mechanism_routes()
        .iter()
        .copied()
        .any(|route| {
            route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        rule_index,
                        target: ThemeTarget::Edge,
                        selector: FamilyThemeSelectorShape::Static { variant: None },
                        facet: FamilyThemeRuleFacet::StrokeWidth,
                    } if rule_index == expected_rule_index
                )
        })
}

fn tree_view_terminal_counts(root: &TreeViewNode) -> (usize, usize) {
    let mut node_count = 0usize;
    let mut line_count = 0usize;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        node_count = node_count.saturating_add(1);
        line_count = line_count.saturating_add(1);
        if !node.children.is_empty() {
            line_count = line_count.saturating_add(1);
        }
        stack.extend(node.children.iter());
    }
    (node_count, line_count)
}

/// Renderer-owned proof for Tree View's final CSS and terminal occurrence streams.
#[derive(Debug)]
pub(crate) struct TreeViewThemeReceipt {
    expected_label_color: Option<Box<str>>,
    expected_line_color: Option<Box<str>>,
    expected_icon_color: Option<Box<str>>,
    expected_font_family: Box<str>,
    expected_label_count: usize,
    expected_icon_count: usize,
    expected_line_count: usize,
    expected_stroke_width: Option<Box<str>>,
    css_seen: bool,
    css_matches: bool,
    labels_seen: usize,
    icons_seen: usize,
    lines_seen: usize,
    terminal_matches: bool,
}

impl TreeViewThemeReceipt {
    fn new(
        expected_label_color: Option<&str>,
        expected_line_color: Option<&str>,
        expected_icon_color: Option<&str>,
        expected_font_family: &str,
        expected_label_count: usize,
        expected_icon_count: usize,
        expected_line_count: usize,
        expected_stroke_width: Option<&str>,
    ) -> Self {
        Self {
            expected_label_color: expected_label_color.map(Into::into),
            expected_line_color: expected_line_color.map(Into::into),
            expected_icon_color: expected_icon_color.map(Into::into),
            expected_font_family: expected_font_family.into(),
            expected_label_count,
            expected_icon_count,
            expected_line_count,
            expected_stroke_width: expected_stroke_width.map(Into::into),
            css_seen: false,
            css_matches: true,
            labels_seen: 0,
            icons_seen: 0,
            lines_seen: 0,
            terminal_matches: true,
        }
    }

    pub(crate) fn record_css(
        &mut self,
        font_family: &str,
        label_color: &str,
        line_color: &str,
        icon_color: &str,
    ) {
        if self.css_seen {
            self.css_matches = false;
            return;
        }
        self.css_seen = true;
        self.css_matches &= font_family == self.expected_font_family.as_ref();
        if let Some(expected) = &self.expected_label_color {
            self.css_matches &= label_color == expected.as_ref();
        }
        if let Some(expected) = &self.expected_line_color {
            self.css_matches &= line_color == expected.as_ref();
        }
        if let Some(expected) = &self.expected_icon_color {
            self.css_matches &= icon_color == expected.as_ref();
        }
    }

    pub(crate) fn record_label(&mut self, emitted_color: &str) {
        self.labels_seen = self.labels_seen.saturating_add(1);
        if let Some(expected) = &self.expected_label_color {
            self.terminal_matches &= emitted_color == expected.as_ref();
        }
    }

    pub(crate) fn record_icon(&mut self, emitted_color: &str) {
        self.icons_seen = self.icons_seen.saturating_add(1);
        if let Some(expected) = &self.expected_icon_color {
            self.terminal_matches &= emitted_color == expected.as_ref();
        }
    }

    pub(crate) fn record_line(&mut self, emitted_color: &str, emitted_stroke_width: Option<&str>) {
        self.lines_seen = self.lines_seen.saturating_add(1);
        if let Some(expected) = &self.expected_line_color {
            self.terminal_matches &= emitted_color == expected.as_ref();
        }
        if let Some(expected) = &self.expected_stroke_width {
            self.terminal_matches &= emitted_stroke_width == Some(expected.as_ref());
        }
    }

    fn proves(&self, plan: &TreeViewThemePlan) -> bool {
        self.css_seen
            && self.css_matches
            && self.labels_seen == self.expected_label_count
            && self.icons_seen == self.expected_icon_count
            && self.lines_seen == self.expected_line_count
            && self.terminal_matches
            && plan.expected_line_count == self.expected_line_count
    }

    fn label_color_matches(&self, expected: &str) -> bool {
        self.css_seen
            && self.css_matches
            && self.labels_seen == self.expected_label_count
            && self.expected_label_color.as_deref() == Some(expected)
    }

    fn line_color_matches(&self, expected: &str) -> bool {
        self.css_seen
            && self.css_matches
            && self.lines_seen == self.expected_line_count
            && self.expected_line_color.as_deref() == Some(expected)
    }

    fn icon_color_matches(&self, expected: &str) -> bool {
        self.css_seen
            && self.css_matches
            && self.icons_seen == self.expected_icon_count
            && self.expected_icon_color.as_deref() == Some(expected)
    }

    fn font_family_matches(&self, expected: &str) -> bool {
        self.css_seen && self.css_matches && self.expected_font_family.as_ref() == expected
    }
}

#[derive(Debug, Default)]
struct TreeViewEdgeRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    stroke_width_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    fn work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    fn resolved_fill(target: ThemeTarget) -> ResolvedDiagramTheme {
        let styles = ThemeRuleSet::default().with_rule(
            ThemeRule::new(
                target,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )
            .for_family(DiagramFamilyId::TREE_VIEW),
        );
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile TreeView terminal fixture")
            .resolve(DiagramFamilyId::TREE_VIEW)
    }

    #[test]
    fn terminal_receipt_requires_css_and_exact_terminal_occurrences() {
        let mut complete = TreeViewThemeReceipt::new(
            Some("#123456"),
            Some("#654321"),
            Some("#abcdef"),
            "Excalifont",
            1,
            1,
            1,
            Some("6"),
        );
        complete.record_css("Excalifont", "#123456", "#654321", "#abcdef");
        complete.record_label("#123456");
        complete.record_icon("#abcdef");
        complete.record_line("#654321", Some("6"));
        assert!(complete.css_matches);
        assert_eq!(complete.labels_seen, 1);
        assert_eq!(complete.icons_seen, 1);
        assert_eq!(complete.lines_seen, 1);

        complete.record_line("#654321", Some("6"));
        assert_eq!(complete.lines_seen, 2);
        assert!(!complete.proves(&TreeViewThemePlan::baseline(1)));

        let mut mismatch =
            TreeViewThemeReceipt::new(None, None, None, "Excalifont", 1, 0, 1, Some("6"));
        mismatch.record_css("Excalifont", "black", "black", "#546e7a");
        mismatch.record_label("black");
        mismatch.record_line("black", Some("5"));
        assert!(!mismatch.proves(&TreeViewThemePlan::baseline(1)));
    }

    #[test]
    fn visible_tree_node_keeps_unsupported_fill_as_a_residual() {
        let theme = resolved_fill(ThemeTarget::Node);
        let model = TreeViewDiagramRenderModel::default();

        let evidence = TreeViewThemePlan::resolve(
            Some(&theme),
            &merman_core::MermaidConfig::default(),
            &model,
            &work_meter(),
        )
        .expect("resolve visible TreeView Node evidence")
        .finish_evidence();

        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].key(),
            &FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }
        );
        assert!(evidence.not_applicable_mechanisms().is_empty());
    }

    #[test]
    fn tree_view_model_title_is_not_a_visible_title_terminal() {
        let theme = resolved_fill(ThemeTarget::Title);
        let model = TreeViewDiagramRenderModel {
            title: Some("Not rendered".to_string()),
            ..Default::default()
        };

        let evidence = TreeViewThemePlan::resolve(
            Some(&theme),
            &merman_core::MermaidConfig::default(),
            &model,
            &work_meter(),
        )
        .expect("resolve terminal-less TreeView title evidence")
        .finish_evidence();

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Title,
            }]
        );
        assert!(evidence.residuals().is_empty());
    }
}
