use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, InheritedFontStackPlan, StaticSurfacePaint, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::IshikawaDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod css_binding;
pub(crate) use css_binding::IshikawaCssBinding;

/// Final Ishikawa text styling shared by terminal SVG emission and family evidence.
///
/// Upstream deterministic spacing and wrapping consume root `fontSize`. The base CSS size remains
/// a separate terminal property, so this plan intentionally does not feed typed CSS typography
/// back into layout.
#[derive(Debug)]
pub(crate) struct IshikawaTextThemePlan {
    css_binding: IshikawaCssBinding,
    text_count: usize,
    base_size_text_count: usize,
    fill: Option<Box<str>>,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Box<str>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    evidence: FamilyThemeOutcomes,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    core_paints: Vec<StaticSurfacePaint>,
    terminal_emitted: OnceLock<()>,
}

impl IshikawaTextThemePlan {
    #[cfg(test)]
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        layout: &IshikawaDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        Self::resolve_with_font_stack(
            theme,
            effective_config,
            layout,
            work_meter,
            inherited_font_stack,
        )
    }

    pub(crate) fn resolve_with_font_stack(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        layout: &IshikawaDiagramLayout,
        work_meter: &OperationWorkMeter,
        inherited_font_stack: InheritedFontStackPlan,
    ) -> Result<Self, OperationWorkError> {
        let (text_count, base_size_text_count) = if theme.is_some() {
            text_occurrences(layout)
        } else {
            (0, 0)
        };
        let mut resolved_fill = None;
        let mut css_binding = IshikawaCssBinding::resolve(effective_config.as_value());
        let typed_font_size_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            })
        });
        let config_owns_font_size = typed_font_size_requested
            && effective_config.config_path_overrides_typed_default("themeVariables.fontSize");
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let font_size_css = match theme {
            Some(theme) if typed_font_size_active => {
                format!("{}px", theme.typography().font_size_px())
            }
            _ => crate::ishikawa::IshikawaConfigView::new(effective_config.as_value())
                .stylesheet_font_size_css()
                .unwrap_or_else(|| format!("{}px", layout.font_size)),
        }
        .into_boxed_str();
        let Some(theme) = theme else {
            return Ok(Self::baseline(
                inherited_font_stack,
                font_size_css,
                css_binding,
            ));
        };

        let mut body_count = usize::from(layout.head.is_some());
        let mut line_count = usize::from(layout.spine.is_some());
        let mut marker_count = 0usize;
        for pair in &layout.pairs {
            for branch in std::iter::once(&pair.upper).chain(pair.lower.as_ref()) {
                body_count += 1;
                line_count += 1 + branch.sub_groups.len();
                marker_count += usize::from(branch.line.marker_start);
                marker_count += branch
                    .sub_groups
                    .iter()
                    .filter(|group| group.line.marker_start)
                    .count();
            }
        }
        if css_binding.look != "handDrawn" {
            marker_count +=
                usize::from(layout.spine.as_ref().is_some_and(|line| line.marker_start));
        }
        let line_owned =
            effective_config.config_paint_overrides_typed_default("themeVariables.lineColor");
        let body_owned =
            effective_config.config_paint_overrides_typed_default("themeVariables.mainBkg");
        let node = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Node,
            &vec![body_owned; body_count],
            &vec![line_owned; body_count],
            work_meter,
        )?;
        let edge = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Edge,
            &vec![true; line_count],
            &vec![line_owned; line_count],
            work_meter,
        )?;
        let marker = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Marker,
            &vec![line_owned; marker_count],
            &vec![line_owned && css_binding.look == "handDrawn"; marker_count],
            work_meter,
        )?;
        if let Some(fill) = &node.fill {
            css_binding.main_bkg = fill.css().to_owned();
        }
        if let Some(stroke) = &node.stroke {
            css_binding.node_stroke = stroke.css().to_owned();
        }
        if let Some(stroke) = &edge.stroke {
            css_binding.line_color = stroke.css().to_owned();
        }
        if let Some(fill) = &marker.fill {
            css_binding.marker_fill = fill.css().to_owned();
        }

        let mermaid_owns_fill = mermaid_owns_text_fill(effective_config);
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut expected_capabilities =
            BTreeMap::<(usize, ResolvedStyleProperty), ThemeCapability>::new();

        if text_count != 0 {
            let style = theme.text_style_with_work_meter(
                ThemeTarget::NodeLabel,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            winner_properties.extend(
                style
                    .winner_rule_properties()
                    .map(|(property, origin)| (origin.rule_index(), property)),
            );
            if !mermaid_owns_fill
                && let Some(fill) = resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::Text, ThemeTarget::NodeLabel],
                    DirectStaticSelectorDomain::Default,
                )
            {
                let (css, rule_index, capability) = fill.into_parts();
                expected_capabilities.insert((rule_index, ResolvedStyleProperty::Fill), capability);
                resolved_fill = Some(css);
            }
        }

        let mut evidence = FamilyThemeOutcomes::default();
        let mut observations = BTreeMap::<(usize, ThemeTarget), TextRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: target @ (ThemeTarget::Text | ThemeTarget::NodeLabel),
                    selector,
                    facet,
                } => {
                    let observation = observations.entry((rule_index, target)).or_default();
                    if !selector_matches_text_domain(selector, text_count) {
                        continue;
                    }
                    let property = resolved_style_property_for_facet(facet);
                    let conservative_unsupported = route.disposition()
                        == FamilyThemeDisposition::Unsupported
                        && matches!(
                            selector,
                            FamilyThemeSelectorShape::Ordinal { .. }
                                | FamilyThemeSelectorShape::Static { variant: Some(_) }
                        );
                    if !winner_properties.contains(&(rule_index, property))
                        && !conservative_unsupported
                    {
                        continue;
                    }

                    observation.applicable = true;
                    if mermaid_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                        continue;
                    }
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static {
                                variant: None | Some(ThemeVariant::Default),
                            },
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some(capability) = expected_capabilities
                                .get(&(rule_index, ResolvedStyleProperty::Fill))
                            {
                                observation.capabilities.insert(*capability);
                            }
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Text,
                } => {
                    // Reconciled below from each visible text terminal's final fill winner.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text,
                    ..
                } => {
                    // Reconciled below from each visible text terminal's final style.
                }
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Title,
                    ..
                }
                | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Title,
                }
                | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Title,
                    ..
                } => {
                    // Ishikawa has no visual title terminal. Root accessibility metadata is not
                    // the semantic Title styling target, so every Title mechanism has zero
                    // occurrences regardless of its matrix disposition.
                    evidence.mark_not_applicable(theme.family_mechanism_key(route));
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let source_owned_fill = vec![mermaid_owns_fill; text_count];
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::textual(
                ThemeTarget::NodeLabel,
                &[ThemeTarget::Text, ThemeTarget::NodeLabel],
                TerminalVariantDomain::uniform(text_count, ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)],
            work_meter,
        )?;

        let mut pending = BTreeMap::new();
        for ((rule_index, target), observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules remain fail-closed until every winning facet has a terminal owner.
            } else if !observation.capabilities.is_empty() {
                pending.insert(key, observation.capabilities);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            css_binding,
            text_count,
            base_size_text_count,
            fill: resolved_fill,
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested,
            typed_font_size_active,
            evidence,
            pending,
            core_paints: vec![node, edge, marker],
            terminal_emitted: OnceLock::new(),
        })
    }

    fn baseline(
        inherited_font_stack: InheritedFontStackPlan,
        font_size_css: Box<str>,
        css_binding: IshikawaCssBinding,
    ) -> Self {
        Self {
            css_binding,
            text_count: 0,
            base_size_text_count: 0,
            fill: None,
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested: false,
            typed_font_size_active: false,
            evidence: FamilyThemeOutcomes::default(),
            pending: BTreeMap::new(),
            core_paints: Vec::new(),
            terminal_emitted: OnceLock::new(),
        }
    }

    pub(crate) fn typed_fill(&self) -> Option<&str> {
        self.fill.as_deref()
    }

    pub(crate) fn write_text_rules(
        &self,
        css: &mut impl std::fmt::Write,
        diagram_id: impl Copy + std::fmt::Display,
        text_color: &str,
    ) {
        let _ = write!(
            css,
            "#{diagram_id} .ishikawa text {{ font-family: {}; font-size: {}; fill: {}; }}\
#{diagram_id} .ishikawa .ishikawa-head-label {{ font-weight: 600; text-anchor: middle; dominant-baseline: middle; font-size: 14px; }}",
            self.inherited_font_stack.font_family_css(),
            self.font_size_css,
            text_color
        );
    }

    pub(crate) fn css_binding(&self) -> &IshikawaCssBinding {
        &self.css_binding
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if self.terminal_emitted.get().is_some() {
            for paint in &self.core_paints {
                paint.record_evidence(evidence);
            }
        }
        evidence.merge_borrowed(&self.evidence);
        for (key, capabilities) in &self.pending {
            evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
        }
        self.record_typography_evidence(evidence);
    }

    pub(crate) fn record_terminal(&self) {
        let _ = self.terminal_emitted.set(());
    }

    fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    fn record_typography_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if !self.typography_requested() {
            return;
        }
        let font_size_has_terminal = self.base_size_text_count != 0;
        self.inherited_font_stack
            .record_unsupported_typography_evidence(evidence, self.text_count != 0);
        for (property, requested, applied) in [
            (
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active && font_size_has_terminal,
            ),
        ] {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            if self.text_count == 0 {
                evidence.mark_not_applicable(key);
            } else if applied {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
    }
}

fn text_occurrences(layout: &IshikawaDiagramLayout) -> (usize, usize) {
    let mut base_size = 0usize;
    for pair in &layout.pairs {
        for branch in std::iter::once(&pair.upper).chain(pair.lower.as_ref()) {
            base_size += usize::from(!branch.label_group.label.text.trim().is_empty());
            base_size += branch
                .sub_groups
                .iter()
                .filter(|group| !group.label.text.trim().is_empty())
                .count();
        }
    }
    let head = usize::from(
        layout
            .head
            .as_ref()
            .is_some_and(|head| !head.label.text.trim().is_empty()),
    );
    (base_size.saturating_add(head), base_size)
}

fn selector_matches_text_domain(selector: FamilyThemeSelectorShape, text_count: usize) -> bool {
    match selector {
        FamilyThemeSelectorShape::Static {
            variant: None | Some(ThemeVariant::Default),
        }
        | FamilyThemeSelectorShape::Ordinal {
            variant: None | Some(ThemeVariant::Default),
            ..
        } => selector.ordinal_domain_intersects_occurrence_count(text_count),
        FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => false,
    }
}

fn mermaid_owns_text_fill(config: &PresentationConfig) -> bool {
    config.config_paint_overrides_typed_default("themeVariables.textColor")
}

#[derive(Debug, Default)]
struct TextRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

#[cfg(test)]
mod tests {
    fn collect_evidence(
        theme: &crate::diagram_theme::ResolvedDiagramTheme,
        record: impl FnOnce(&mut crate::family::FamilyThemeEvidence),
    ) -> crate::family::FamilyThemeEvidence {
        let mut evidence = crate::family::FamilyThemeEvidence::from_theme(Some(theme));
        record(&mut evidence);
        evidence
    }

    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding,
        EffectGraph, EffectInput, EffectPrimitive, OrdinalPalette, ThemeColorValue, ThemeRule,
        ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;
    use serde_json::json;

    #[test]
    fn absent_title_terminal_accounts_every_title_mechanism_as_not_applicable() {
        let palette = OrdinalPalette::new([
            ThemeColorValue::parse("#123456").expect("valid Ishikawa title palette color")
        ])
        .expect("non-empty Ishikawa title palette");
        let effect_id = "ishikawa-title-shadow";
        let effect = EffectGraph::new(
            effect_id,
            [EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: 1.0,
                offset_y: 1.0,
                blur_radius: 2.0,
                spread: 0.0,
                color: ThemeColorValue::parse("#00000080")
                    .expect("valid Ishikawa title shadow color"),
            }],
        )
        .expect("valid Ishikawa title effect");
        let effects = DiagramEffectSet::default()
            .with_graph(effect)
            .expect("unique Ishikawa title effect")
            .with_binding(
                EffectBinding::new(ThemeTarget::Title, effect_id)
                    .expect("valid Ishikawa title effect binding"),
            )
            .expect("unique Ishikawa title effect binding");
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_styles(
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(
                                ThemeTarget::Title,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid("#abcdef")
                                        .expect("valid Ishikawa title fill"),
                                ),
                            ))
                            .with_ordinal_palette(ThemeTarget::Title, palette),
                    )
                    .with_effects(effects),
            )
            .expect("compile Ishikawa title-only theme")
            .resolve(DiagramFamilyId::ISHIKAWA);
        let layout = IshikawaDiagramLayout {
            bounds: None,
            total_width: 0.0,
            total_height: 0.0,
            viewbox_x: 0.0,
            viewbox_y: 0.0,
            padding: 0.0,
            use_max_width: false,
            font_size: 16.0,
            head: None,
            spine: None,
            pairs: Vec::new(),
        };
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let config = crate::test_support::presentation_config(json!({}));

        let plan = IshikawaTextThemePlan::resolve(Some(&resolved), &config, &layout, &work_meter)
            .expect("resolve Ishikawa title-only theme");
        let mut css = String::new();
        plan.write_text_rules(&mut css, "ishikawa-title", "#333333");

        let evidence = collect_evidence(&resolved, |evidence| plan.record_evidence(evidence));
        let expected = [
            FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Title,
            },
            FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Title,
            },
            FamilyThemeMechanismKey::EffectBinding {
                target: ThemeTarget::Title,
                effect_id: effect_id.to_string(),
            },
        ];
        assert_eq!(evidence.not_applicable_mechanisms().len(), expected.len());
        for key in expected {
            assert!(evidence.not_applicable_mechanisms().contains(&key));
        }
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
    }
}
