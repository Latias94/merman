use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::pie::PieDiagramRenderModel;

use crate::config::config_string;
use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod css_binding;
pub(crate) use css_binding::PieCssBinding;

const MERMAID_PIE_PALETTE_SIZE: usize = 12;
const PIE_SLICE_CLASS: &str = "pieCircle";
const PIE_OUTER_CIRCLE_CLASS: &str = "pieOuterCircle";
pub(crate) const PIE_TITLE_CLASS: &str = "pieTitleText";
const PIE_SLICE_STROKE_PATH: &str = "themeVariables.pieStrokeColor";
const PIE_OUTER_STROKE_PATH: &str = "themeVariables.pieOuterStrokeColor";
const PIE_TITLE_FILL_PATH: &str = "themeVariables.pieTitleTextColor";
const PIE_SECTION_TEXT_FILL_PATH: &str = "themeVariables.pieSectionTextColor";
const MERMAID_PIE_SLOT_KEYS: [&str; MERMAID_PIE_PALETTE_SIZE] = [
    "pie1", "pie2", "pie3", "pie4", "pie5", "pie6", "pie7", "pie8", "pie9", "pie10", "pie11",
    "pie12",
];
const MERMAID_PIE_SLOT_PATHS: [&str; MERMAID_PIE_PALETTE_SIZE] = [
    "themeVariables.pie1",
    "themeVariables.pie2",
    "themeVariables.pie3",
    "themeVariables.pie4",
    "themeVariables.pie5",
    "themeVariables.pie6",
    "themeVariables.pie7",
    "themeVariables.pie8",
    "themeVariables.pie9",
    "themeVariables.pie10",
    "themeVariables.pie11",
    "themeVariables.pie12",
];

#[derive(Debug)]
struct PieSlicePaint {
    fill: String,
    typed_owner: Option<PieSlicePaintOwner>,
}

#[derive(Debug, Clone, Copy)]
enum PieSlicePaintOwner {
    Palette(ThemeCapability),
    Rule {
        rule_index: usize,
        capability: ThemeCapability,
    },
}

#[derive(Debug, Clone)]
struct PieSliceStroke {
    css: Box<str>,
    rule_index: usize,
    capability: ThemeCapability,
    slice_site: bool,
    outer_site: bool,
}

#[derive(Debug, Clone)]
struct PieTitleFill {
    css: Box<str>,
    rule_index: usize,
    capability: ThemeCapability,
}

#[derive(Debug, Clone)]
struct PieTextFill {
    css: Box<str>,
    rule_index: usize,
    capability: ThemeCapability,
}

#[derive(Debug, Default)]
struct PieTerminalEvidence {
    palette_capabilities: BTreeSet<ThemeCapability>,
    fill_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    stroke_applied: bool,
    stroke_not_applicable: bool,
    text_fill_applied: bool,
    text_fill_not_applicable: bool,
    typography_applied: bool,
    typography_not_applicable: bool,
}

/// Resolves Pie theme surfaces once for layout, SVG emission, and terminal evidence.
#[derive(Debug)]
pub(crate) struct PieThemePlan {
    css: PieCssBinding,
    label_indices: HashMap<String, usize>,
    section_paint_indices: Vec<usize>,
    paints: Vec<PieSlicePaint>,
    stroke: Option<PieSliceStroke>,
    title_fill: Option<PieTitleFill>,
    text_fill: Option<PieTextFill>,
    inherited_font_stack: InheritedFontStackPlan,
    title_present: bool,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    pending_rules: BTreeMap<FamilyThemeMechanismKey, PiePendingEvidence>,
    terminal_evidence: OnceLock<PieTerminalEvidence>,
}

impl PieThemePlan {
    #[cfg(test)]
    pub(crate) fn baseline(
        model: &PieDiagramRenderModel,
        effective_config: &serde_json::Value,
    ) -> Self {
        let effective_config = MermaidConfig::from_value(effective_config.clone());
        Self::baseline_with_config(model, &effective_config, None)
    }

    fn baseline_with_config(
        model: &PieDiagramRenderModel,
        effective_config: &MermaidConfig,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> Self {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let css = PieCssBinding::new(
            effective_config.as_value(),
            inherited_font_stack
                .typography_requested()
                .then_some(inherited_font_stack.font_family_css()),
        );
        let mut plan = Self {
            css,
            label_indices: HashMap::new(),
            section_paint_indices: Vec::with_capacity(model.sections.len()),
            paints: Vec::new(),
            stroke: None,
            title_fill: None,
            text_fill: None,
            inherited_font_stack,
            title_present: false,
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            pending_rules: BTreeMap::new(),
            terminal_evidence: OnceLock::new(),
        };
        for section in &model.sections {
            let paint_index =
                plan.insert_mermaid_paint(&section.label, effective_config.as_value());
            plan.section_paint_indices.push(paint_index);
        }
        plan
    }

    #[cfg(test)]
    pub(crate) fn resolve(
        model: &PieDiagramRenderModel,
        effective_config: &MermaidConfig,
        theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        Self::resolve_with_title(
            model,
            effective_config,
            theme,
            model.title.as_deref(),
            work_meter,
        )
    }

    pub(crate) fn resolve_with_title(
        model: &PieDiagramRenderModel,
        effective_config: &MermaidConfig,
        theme: Option<&ResolvedDiagramTheme>,
        title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline_with_config(model, effective_config, theme);
        plan.title_present = title.is_some_and(|title| !title.trim().is_empty());
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        plan.resolve_palette(effective_config, theme, work_meter)?;
        plan.resolve_rules(model, effective_config, theme, work_meter)?;
        plan.resolve_title_rules(effective_config, theme, title, work_meter)?;
        plan.resolve_text_rules(effective_config, theme, model.sections.len(), work_meter)?;
        plan.bind_css_winners();
        Ok(plan)
    }

    fn bind_css_winners(&mut self) {
        if let Some(stroke) = self.stroke.as_ref() {
            if stroke.slice_site {
                self.css.slice_stroke.bind_typed(&stroke.css);
            }
            if stroke.outer_site {
                self.css.outer_stroke.bind_typed(&stroke.css);
            }
        }
        if let Some(fill) = self.title_fill.as_ref() {
            self.css.title_text_color = fill.css.to_string();
        }
        if let Some(fill) = self.text_fill.as_ref() {
            self.css.section_text_color = fill.css.to_string();
        }
    }

    pub(crate) fn css_binding(&self) -> &PieCssBinding {
        &self.css
    }

    fn resolve_palette(
        &mut self,
        effective_config: &MermaidConfig,
        theme: &ResolvedDiagramTheme,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::PieSlice) else {
            return Ok(());
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::PieSlice,
        };
        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                self.palette_key = Some(key.clone());
                let mut missing_typed_color = false;
                for (index, paint) in self.paints.iter_mut().enumerate() {
                    if merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        MERMAID_PIE_SLOT_PATHS[index % MERMAID_PIE_PALETTE_SIZE],
                    ) {
                        continue;
                    }
                    let style = theme.style_with_work_meter(
                        ThemeTarget::PieSlice,
                        ThemeVariant::Default,
                        Some(index + 1),
                        work_meter,
                    )?;
                    if style.fill_resolution().winner().is_some() {
                        continue;
                    }
                    let Some(color) = theme.series_color(ThemeTarget::PieSlice, index + 1) else {
                        missing_typed_color = true;
                        continue;
                    };
                    paint.fill = color.as_css();
                    paint.typed_owner =
                        Some(PieSlicePaintOwner::Palette(if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        }));
                }
                if missing_typed_color {
                    self.evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
                    self.palette_key = None;
                }
            }
            FamilyThemeDisposition::Unsupported => {
                self.evidence
                    .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
            }
            FamilyThemeDisposition::LegacyCompatibility => {}
        }
        Ok(())
    }

    fn resolve_rules(
        &mut self,
        model: &PieDiagramRenderModel,
        effective_config: &MermaidConfig,
        theme: &ResolvedDiagramTheme,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let mut occurrence_winner_properties = BTreeSet::new();
        let mut typed_fill_rules = BTreeSet::new();
        let mut suppressed_fill_rules = BTreeSet::new();
        let needs_typed_fill_resolution =
            theme
                .family_mechanism_routes()
                .iter()
                .any(|route| match route.mechanism() {
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::PieSlice,
                        selector: FamilyThemeSelectorShape::Ordinal { .. },
                        facet: FamilyThemeRuleFacet::Fill(_),
                        ..
                    } => true,
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::PieSlice,
                        selector:
                            FamilyThemeSelectorShape::Static {
                                variant: None | Some(ThemeVariant::Default),
                            },
                        facet:
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ..
                    } => route.disposition() == FamilyThemeDisposition::TypedAdapter,
                    _ => false,
                });
        if needs_typed_fill_resolution {
            for (index, paint) in self.paints.iter_mut().enumerate() {
                let style = theme.style_with_work_meter(
                    ThemeTarget::PieSlice,
                    ThemeVariant::Default,
                    Some(index + 1),
                    work_meter,
                )?;
                occurrence_winner_properties.extend(
                    style
                        .winner_rule_properties()
                        .map(|(property, origin)| (origin.rule_index(), property)),
                );
                let Some(fill) = resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::PieSlice],
                    DirectStaticSelectorDomain::Default,
                ) else {
                    continue;
                };
                if merman_core::__private::config_path_overrides_typed_default(
                    effective_config,
                    MERMAID_PIE_SLOT_PATHS[index % MERMAID_PIE_PALETTE_SIZE],
                ) {
                    suppressed_fill_rules.insert(fill.rule_index());
                    continue;
                }
                paint.fill = fill.css().to_owned();
                paint.typed_owner = Some(PieSlicePaintOwner::Rule {
                    rule_index: fill.rule_index(),
                    capability: fill.capability(),
                });
                typed_fill_rules.insert(fill.rule_index());
            }
        }

        let slice_config_owned =
            explicit_config_owns_stroke_site(effective_config, PIE_SLICE_STROKE_PATH);
        let outer_config_owned =
            explicit_config_owns_stroke_site(effective_config, PIE_OUTER_STROKE_PATH);
        let slice_site = !mermaid_owns_stroke_site(effective_config, PIE_SLICE_STROKE_PATH);
        let outer_site = !mermaid_owns_stroke_site(effective_config, PIE_OUTER_STROKE_PATH);
        let style = theme.style_with_work_meter(
            ThemeTarget::PieSlice,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let occurrence_count = model.sections.len();
        let winner_properties =
            if needs_typed_fill_resolution && self.paints.len() == occurrence_count {
                occurrence_winner_properties
            } else {
                pie_occurrence_winner_properties(
                    theme,
                    ThemeTarget::PieSlice,
                    occurrence_count,
                    &style,
                    work_meter,
                )?
            };
        self.stroke = typed_static_stroke(theme, &style, slice_site, outer_site);

        let mut observations = BTreeMap::<usize, PieSliceRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::PieSlice,
                selector,
                facet,
            } = route.mechanism()
            else {
                continue;
            };
            let observation = observations.entry(rule_index).or_default();
            if !selector.ordinal_domain_intersects_occurrence_count(occurrence_count) {
                continue;
            }
            let route_won =
                winner_properties.contains(&(rule_index, resolved_style_property_for_facet(facet)));
            if !route_won {
                continue;
            }

            observation.applicable = true;
            // A surviving legacy projection also outranks typed defaults. It must not classify
            // its own legacy route as NotApplicable unless explicit config owns both sites.
            let stroke_is_owned_elsewhere = (slice_config_owned && outer_config_owned)
                || (route.disposition() != FamilyThemeDisposition::LegacyCompatibility
                    && !slice_site
                    && !outer_site);
            if matches!(facet, FamilyThemeRuleFacet::Stroke(_)) && stroke_is_owned_elsewhere {
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
                ) if route_won => {
                    if occurrence_count == 0 {
                        // No slice path or legend swatch exists for this semantic surface.
                    } else if typed_fill_rules.contains(&rule_index) {
                        observation.pending.fill = true;
                    } else if !suppressed_fill_rules.contains(&rule_index) {
                        observation.incomplete = true;
                    }
                }
                (
                    FamilyThemeDisposition::TypedAdapter,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                    FamilyThemeRuleFacet::Stroke(
                        FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                    ),
                ) if self
                    .stroke
                    .as_ref()
                    .is_some_and(|stroke| stroke.rule_index == rule_index) =>
                {
                    observation.pending.stroke = true;
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

        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::PieSlice,
            };
            if !observation.applicable {
                self.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                self.evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules remain fail-closed until every winning facet has a terminal owner.
            } else if observation.pending.requires_terminal_proof() {
                self.pending_rules.insert(key, observation.pending);
            } else {
                self.evidence.mark_not_applicable(key);
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut self.evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::PieSlice,
                TerminalVariantDomain::uniform(occurrence_count, ThemeVariant::Default),
            )],
            work_meter,
        )?;
        Ok(())
    }

    fn resolve_title_rules(
        &mut self,
        effective_config: &MermaidConfig,
        theme: &ResolvedDiagramTheme,
        title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let title_present = title.is_some_and(|title| !title.trim().is_empty());
        let title_count = usize::from(title_present);
        let config_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            PIE_TITLE_FILL_PATH,
        );
        let style = title_present
            .then(|| {
                theme.style_with_work_meter(
                    ThemeTarget::Title,
                    ThemeVariant::Default,
                    Some(1),
                    work_meter,
                )
            })
            .transpose()?;
        let winner_properties = style
            .as_ref()
            .into_iter()
            .flat_map(|style| style.winner_rule_properties())
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let typed_fill = (!config_owns_fill)
            .then(|| {
                style.as_ref().and_then(|style| {
                    resolve_direct_static_fill(
                        theme,
                        style,
                        &[ThemeTarget::Title],
                        DirectStaticSelectorDomain::Default,
                    )
                })
            })
            .flatten();
        self.title_fill = typed_fill.map(|fill| {
            let (css, rule_index, capability) = fill.into_parts();
            PieTitleFill {
                css,
                rule_index,
                capability,
            }
        });
        let typed_fill_rule = self.title_fill.as_ref().map(|fill| fill.rule_index);
        let source_owned_fill = [config_owns_fill];

        let mut observations = BTreeMap::<usize, PieTitleRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Title,
                selector,
                facet,
            } = route.mechanism()
            else {
                continue;
            };
            let observation = observations.entry(rule_index).or_default();
            if !selector.ordinal_domain_intersects_occurrence_count(title_count) {
                continue;
            }
            let route_won =
                winner_properties.contains(&(rule_index, resolved_style_property_for_facet(facet)));
            if !route_won {
                continue;
            }

            observation.applicable = true;
            if config_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                continue;
            }
            match (route.disposition(), selector, facet) {
                (
                    FamilyThemeDisposition::TypedAdapter,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                    FamilyThemeRuleFacet::Fill(_),
                ) if typed_fill_rule == Some(rule_index) => {
                    observation.fill_pending = true;
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

        reconcile_unsupported_terminal_domains(
            theme,
            &mut self.evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Title,
                TerminalVariantDomain::uniform(title_count, ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)],
            work_meter,
        )?;

        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Title,
            };
            if !observation.applicable {
                self.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                self.evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules remain fail-closed until every winning facet has a terminal owner.
            } else if observation.fill_pending {
                self.pending_rules
                    .insert(key, PiePendingEvidence::fill_only());
            } else {
                self.evidence.mark_not_applicable(key);
            }
        }
        Ok(())
    }

    fn resolve_text_rules(
        &mut self,
        effective_config: &MermaidConfig,
        theme: &ResolvedDiagramTheme,
        occurrence_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let config_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            PIE_SECTION_TEXT_FILL_PATH,
        );
        // The writer emits one shared CSS rule. Ordinal requests are observed below, but
        // cannot select this whole-surface fallback on behalf of just the first text node.
        let style = theme.style_with_work_meter(
            ThemeTarget::Text,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let winner_properties = pie_occurrence_winner_properties(
            theme,
            ThemeTarget::Text,
            occurrence_count,
            &style,
            work_meter,
        )?;
        let typed_fill = (!config_owns_fill)
            .then(|| {
                resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::Text],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten();
        self.text_fill = typed_fill.map(|fill| {
            let (css, rule_index, capability) = fill.into_parts();
            PieTextFill {
                css,
                rule_index,
                capability,
            }
        });
        let typed_fill_rule = self.text_fill.as_ref().map(|fill| fill.rule_index);

        let mut pending = None;
        let mut observations = BTreeMap::<usize, PieTextRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Text,
                selector,
                facet,
            } = route.mechanism()
            else {
                continue;
            };
            let observation = observations.entry(rule_index).or_default();
            if !selector.ordinal_domain_intersects_occurrence_count(occurrence_count) {
                continue;
            }
            let route_won =
                winner_properties.contains(&(rule_index, resolved_style_property_for_facet(facet)));
            if !route_won {
                continue;
            }

            observation.applicable = true;
            if config_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
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
                ) if typed_fill_rule == Some(rule_index) => {
                    observation.fill_pending = true;
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

        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Text,
            };
            if !observation.applicable {
                self.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                self.evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules remain fail-closed until every winning facet is owned.
            } else if observation.fill_pending {
                pending = Some(key);
            } else {
                self.evidence.mark_not_applicable(key);
            }
        }
        if let Some(key) = pending {
            self.pending_rules
                .insert(key, PiePendingEvidence::text_fill_only());
        }
        Ok(())
    }

    fn insert_mermaid_paint(&mut self, label: &str, effective_config: &serde_json::Value) -> usize {
        if let Some(index) = self.label_indices.get(label).copied() {
            return index;
        }
        let index = self.paints.len();
        let slot = index % MERMAID_PIE_PALETTE_SIZE;
        let fill = config_string(
            effective_config,
            &["themeVariables", MERMAID_PIE_SLOT_KEYS[slot]],
        )
        .unwrap_or_else(|| default_pie_palette()[slot].to_string());
        self.label_indices.insert(label.to_string(), index);
        self.paints.push(PieSlicePaint {
            fill,
            typed_owner: None,
        });
        index
    }

    pub(crate) fn fill_for(&self, label: &str) -> Option<&str> {
        let index = *self.label_indices.get(label)?;
        self.paints.get(index).map(|paint| paint.fill.as_str())
    }

    pub(crate) fn slice_stroke_css(&self) -> Option<&str> {
        self.stroke
            .as_ref()
            .filter(|stroke| stroke.slice_site)
            .map(|stroke| stroke.css.as_ref())
    }

    pub(crate) fn outer_stroke_css(&self) -> Option<&str> {
        self.stroke
            .as_ref()
            .filter(|stroke| stroke.outer_site)
            .map(|stroke| stroke.css.as_ref())
    }

    pub(crate) fn title_fill_css(&self) -> Option<&str> {
        self.title_fill.as_ref().map(|fill| fill.css.as_ref())
    }

    pub(crate) fn text_fill_css(&self) -> Option<&str> {
        self.text_fill.as_ref().map(|fill| fill.css.as_ref())
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested()
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &self,
        visible_slice_labels: impl IntoIterator<Item = &'a str>,
    ) -> Option<PieThemeReceipt> {
        (self.palette_key.is_some()
            || !self.pending_rules.is_empty()
            || self.stroke.is_some()
            || self.text_fill.is_some()
            || self.typography_requested())
        .then(|| PieThemeReceipt::new(self, visible_slice_labels))
    }

    pub(crate) fn record_terminal(&self, receipt: PieThemeReceipt) -> bool {
        if !receipt.proves_complete(self) {
            return false;
        }
        self.terminal_evidence
            .set(receipt.into_terminal_evidence(self))
            .is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        let Some(terminal) = self.terminal_evidence.get() else {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(&mut evidence, true);
            if self.inherited_font_stack.typed_font_stack_requested() {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            return evidence;
        };
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, terminal.typography_applied);
        if self.inherited_font_stack.typed_font_stack_requested() {
            if terminal.typography_not_applicable {
                evidence.mark_not_applicable(key);
            } else if self.inherited_font_stack.typed_font_stack_active()
                && terminal.typography_applied
            {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else if self.inherited_font_stack.outcome() == InheritedFontStackOutcome::ConfigOwned
                && terminal.typography_applied
            {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
        }
        if let Some(key) = self.palette_key.clone() {
            match self.terminal_evidence.get() {
                Some(terminal) if terminal.palette_capabilities.is_empty() => {
                    evidence.mark_not_applicable(key)
                }
                Some(terminal) => evidence.mark_applied_with_capabilities(
                    key,
                    terminal.palette_capabilities.iter().copied(),
                ),
                None => evidence
                    .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
            }
        }
        if let Some(terminal) = self.terminal_evidence.get() {
            for (key, pending) in &self.pending_rules {
                let FamilyThemeMechanismKey::Rule { index, target } = key else {
                    continue;
                };
                let mut capabilities = BTreeSet::new();
                let mut complete = true;
                if pending.fill {
                    if let Some(fill_capabilities) = terminal.fill_capabilities.get(index) {
                        capabilities.extend(fill_capabilities.iter().copied());
                    } else {
                        complete = false;
                    }
                }
                if pending.text_fill && *target == ThemeTarget::Text {
                    if terminal.text_fill_applied {
                        if let Some(text_fill) = self.text_fill.as_ref() {
                            capabilities.insert(text_fill.capability);
                        }
                    } else if !terminal.text_fill_not_applicable {
                        complete = false;
                    }
                }
                if pending.stroke {
                    if terminal.stroke_applied
                        && self
                            .stroke
                            .as_ref()
                            .is_some_and(|stroke| stroke.rule_index == *index)
                    {
                        if let Some(stroke) = self.stroke.as_ref() {
                            capabilities.insert(stroke.capability);
                        }
                    } else if !terminal.stroke_not_applicable {
                        complete = false;
                    }
                }
                if complete {
                    if capabilities.is_empty() {
                        evidence.mark_not_applicable(key.clone());
                    } else {
                        evidence.mark_applied_with_capabilities(
                            key.clone(),
                            capabilities.iter().copied(),
                        );
                    }
                }
            }
        }
        evidence
    }
}

/// Writer-owned proof that Pie slice, legend, and typed stroke CSS reached their terminal seams.
#[derive(Debug)]
pub(crate) struct PieThemeReceipt {
    expected_slice_paints: Vec<Option<usize>>,
    expected_legend_paints: Vec<Option<usize>>,
    next_slice_index: usize,
    next_legend_index: usize,
    next_slice_text_index: usize,
    values_match: bool,
    slice_classes_match: bool,
    slice_texts_match: bool,
    outer_circle_class: Option<Box<str>>,
    title_stylesheet_fill: Option<Box<str>>,
    title_stylesheet_class: Option<Box<str>>,
    text_stylesheet_fill: Option<Box<str>>,
    text_stylesheet_class: Option<Box<str>>,
    typography_stylesheet_family: Option<Box<str>>,
    visible_title_count: usize,
    title_text_recorded: bool,
    visible_slice_text_count: usize,
    next_legend_text_index: usize,
    visible_legend_text_count: usize,
    legend_texts_match: bool,
    palette_capabilities: BTreeSet<ThemeCapability>,
    fill_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    slice_stroke_css: Option<Box<str>>,
    outer_stroke_css: Option<Box<str>>,
}

impl PieThemeReceipt {
    fn new<'a>(
        plan: &PieThemePlan,
        visible_slice_labels: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        let mut values_match = true;
        let expected_slice_paints = visible_slice_labels
            .into_iter()
            .map(|label| {
                let paint_index = plan.label_indices.get(label).copied();
                values_match &= paint_index.is_some();
                paint_index
            })
            .collect();
        Self {
            expected_slice_paints,
            expected_legend_paints: plan
                .section_paint_indices
                .iter()
                .copied()
                .map(Some)
                .collect(),
            next_slice_index: 0,
            next_legend_index: 0,
            next_slice_text_index: 0,
            values_match,
            slice_classes_match: true,
            slice_texts_match: true,
            outer_circle_class: None,
            title_stylesheet_fill: None,
            title_stylesheet_class: None,
            text_stylesheet_fill: None,
            text_stylesheet_class: None,
            typography_stylesheet_family: None,
            visible_title_count: 0,
            title_text_recorded: false,
            visible_slice_text_count: 0,
            next_legend_text_index: 0,
            visible_legend_text_count: 0,
            legend_texts_match: true,
            palette_capabilities: BTreeSet::new(),
            fill_capabilities: BTreeMap::new(),
            slice_stroke_css: None,
            outer_stroke_css: None,
        }
    }

    pub(crate) fn record_slice(
        &mut self,
        plan: &PieThemePlan,
        slice_index: usize,
        emitted_label: &str,
        emitted_fill: Option<&str>,
        emitted_class: Option<&str>,
    ) {
        let (matches, owner) = record_terminal_paint(
            plan,
            &self.expected_slice_paints,
            &mut self.next_slice_index,
            slice_index,
            emitted_label,
            emitted_fill,
        );
        self.values_match &= matches;
        self.slice_classes_match &= emitted_class.is_some_and(|class| {
            class
                .split_ascii_whitespace()
                .any(|part| part == PIE_SLICE_CLASS)
        });
        if matches {
            self.record_paint_owner(owner);
        }
    }

    pub(crate) fn record_legend(
        &mut self,
        plan: &PieThemePlan,
        legend_index: usize,
        emitted_label: &str,
        emitted_fill: Option<&str>,
    ) {
        let (matches, owner) = record_terminal_paint(
            plan,
            &self.expected_legend_paints,
            &mut self.next_legend_index,
            legend_index,
            emitted_label,
            emitted_fill,
        );
        self.values_match &= matches;
        if matches {
            self.record_paint_owner(owner);
        }
    }

    pub(crate) fn record_text_stylesheet(
        &mut self,
        plan: &PieThemePlan,
        emitted_class: &str,
        emitted_fill: Option<&str>,
    ) {
        let Some(text_fill) = plan.text_fill.as_ref() else {
            self.values_match = false;
            return;
        };
        if self.text_stylesheet_fill.is_some() || self.text_stylesheet_class.is_some() {
            self.values_match = false;
            return;
        }
        self.values_match &= emitted_class == "slice";
        self.values_match &= emitted_fill == Some(text_fill.css.as_ref());
        self.text_stylesheet_class = Some(emitted_class.into());
        self.text_stylesheet_fill = emitted_fill.map(Into::into);
    }

    pub(crate) fn record_slice_text(&mut self, emitted_class: Option<&str>, emitted_text: &str) {
        self.values_match &= self.next_slice_text_index < self.expected_slice_paints.len();
        self.next_slice_text_index = self.next_slice_text_index.saturating_add(1);
        self.slice_texts_match &= emitted_class == Some("slice");
        self.visible_slice_text_count = self
            .visible_slice_text_count
            .saturating_add(usize::from(!emitted_text.trim().is_empty()));
    }

    pub(crate) fn record_legend_text(
        &mut self,
        emitted_parent_class: Option<&str>,
        emitted_text: &str,
    ) {
        self.values_match &= self.next_legend_text_index < self.expected_legend_paints.len();
        self.next_legend_text_index = self.next_legend_text_index.saturating_add(1);
        self.legend_texts_match &= emitted_parent_class == Some("legend");
        self.visible_legend_text_count = self
            .visible_legend_text_count
            .saturating_add(usize::from(!emitted_text.trim().is_empty()));
    }

    pub(crate) fn record_typography_stylesheet(&mut self, emitted_font_family: Option<&str>) {
        if self.typography_stylesheet_family.is_some() {
            self.values_match = false;
            return;
        }
        self.typography_stylesheet_family = emitted_font_family.map(Into::into);
    }

    fn record_paint_owner(&mut self, owner: Option<PieSlicePaintOwner>) {
        match owner {
            Some(PieSlicePaintOwner::Palette(capability)) => {
                self.palette_capabilities.insert(capability);
            }
            Some(PieSlicePaintOwner::Rule {
                rule_index,
                capability,
            }) => {
                self.fill_capabilities
                    .entry(rule_index)
                    .or_default()
                    .insert(capability);
            }
            None => {}
        }
    }

    pub(crate) fn record_slice_stroke_stylesheet(
        &mut self,
        plan: &PieThemePlan,
        emitted_class: &str,
        emitted_stroke: Option<&str>,
    ) {
        let Some(stroke) = plan.stroke.as_ref().filter(|stroke| stroke.slice_site) else {
            self.values_match = false;
            return;
        };
        if self.slice_stroke_css.is_some() {
            self.values_match = false;
            return;
        }
        self.values_match &= emitted_class == PIE_SLICE_CLASS;
        self.values_match &= emitted_stroke == Some(stroke.css.as_ref());
        self.slice_stroke_css = emitted_stroke.map(Into::into);
    }

    pub(crate) fn record_outer_stroke_stylesheet(
        &mut self,
        plan: &PieThemePlan,
        emitted_class: &str,
        emitted_stroke: Option<&str>,
    ) {
        let Some(stroke) = plan.stroke.as_ref().filter(|stroke| stroke.outer_site) else {
            self.values_match = false;
            return;
        };
        if self.outer_stroke_css.is_some() {
            self.values_match = false;
            return;
        }
        self.values_match &= emitted_class == PIE_OUTER_CIRCLE_CLASS;
        self.values_match &= emitted_stroke == Some(stroke.css.as_ref());
        self.outer_stroke_css = emitted_stroke.map(Into::into);
    }

    pub(crate) fn record_outer_circle(&mut self, emitted_class: Option<&str>) {
        if self.outer_circle_class.is_some() {
            self.values_match = false;
            return;
        }
        self.values_match &= emitted_class == Some(PIE_OUTER_CIRCLE_CLASS);
        self.outer_circle_class = emitted_class.map(Into::into);
    }

    pub(crate) fn record_title_stylesheet(
        &mut self,
        plan: &PieThemePlan,
        emitted_class: &str,
        emitted_fill: Option<&str>,
    ) {
        let Some(title_fill) = plan.title_fill.as_ref() else {
            self.values_match = false;
            return;
        };
        if self.title_stylesheet_fill.is_some() || self.title_stylesheet_class.is_some() {
            self.values_match = false;
            return;
        }
        self.values_match &= emitted_class == PIE_TITLE_CLASS;
        self.values_match &= emitted_fill == Some(title_fill.css.as_ref());
        self.title_stylesheet_class = Some(emitted_class.into());
        self.title_stylesheet_fill = emitted_fill.map(Into::into);
    }

    pub(crate) fn record_title_text(
        &mut self,
        emitted_class: Option<&str>,
        emitted_text: Option<&str>,
    ) {
        if self.title_text_recorded {
            self.values_match = false;
        }
        self.title_text_recorded = true;
        self.visible_title_count = self.visible_title_count.saturating_add(usize::from(
            emitted_text.is_some_and(|text| !text.trim().is_empty()),
        ));
        self.values_match &= emitted_class == Some(PIE_TITLE_CLASS);
    }

    fn proves_complete(&self, plan: &PieThemePlan) -> bool {
        let stroke_is_applicable = plan.stroke.is_some() && !self.expected_slice_paints.is_empty();
        let title_is_applicable = plan.title_fill.is_some();
        let text_fill_is_applicable = plan.text_fill.is_some();
        let stroke_sites_match = plan.stroke.as_ref().is_none_or(|stroke| {
            if !stroke_is_applicable {
                return self.slice_stroke_css.is_none() && self.outer_stroke_css.is_none();
            }
            (!stroke.slice_site || self.slice_stroke_css.as_deref() == Some(stroke.css.as_ref()))
                && (!stroke.outer_site
                    || self.outer_stroke_css.as_deref() == Some(stroke.css.as_ref()))
        });
        self.values_match
            && self.next_slice_index == self.expected_slice_paints.len()
            && self.next_legend_index == self.expected_legend_paints.len()
            && (!stroke_is_applicable || self.slice_classes_match)
            && (!stroke_is_applicable
                || plan.stroke.as_ref().is_none_or(|stroke| !stroke.outer_site)
                || self.outer_circle_class.as_deref() == Some(PIE_OUTER_CIRCLE_CLASS))
            && stroke_sites_match
            && (!title_is_applicable
                || plan.title_fill.as_ref().is_some_and(|title_fill| {
                    self.title_stylesheet_fill.as_deref() == Some(title_fill.css.as_ref())
                        && self.title_stylesheet_class.as_deref() == Some(PIE_TITLE_CLASS)
                        && self.visible_title_count == 1
                }))
            && (!text_fill_is_applicable
                || (self.text_stylesheet_class.as_deref() == Some("slice")
                    && self.text_stylesheet_fill.as_deref()
                        == plan
                            .text_fill
                            .as_ref()
                            .map(|text_fill| text_fill.css.as_ref())
                    && self.next_slice_text_index == self.expected_slice_paints.len()
                    && self.slice_texts_match))
            && self.proves_typography(plan)
    }

    fn proves_typography(&self, plan: &PieThemePlan) -> bool {
        if !plan.typography_requested() {
            return true;
        }
        self.typography_stylesheet_family.as_deref() == Some(plan.font_family_css())
            && self.next_slice_text_index == self.expected_slice_paints.len()
            && self.next_legend_text_index == self.expected_legend_paints.len()
            && self.legend_texts_match
            && self.title_text_recorded
            && self.visible_title_count == usize::from(plan.title_present)
    }

    fn into_terminal_evidence(self, plan: &PieThemePlan) -> PieTerminalEvidence {
        let stroke_is_applicable = plan.stroke.is_some() && !self.expected_slice_paints.is_empty();
        let mut fill_capabilities = self.fill_capabilities;
        if let Some(title_fill) = plan.title_fill.as_ref() {
            fill_capabilities
                .entry(title_fill.rule_index)
                .or_default()
                .insert(title_fill.capability);
        }
        PieTerminalEvidence {
            palette_capabilities: self.palette_capabilities,
            fill_capabilities,
            stroke_applied: plan.pending_rules.values().any(|pending| pending.stroke)
                && stroke_is_applicable,
            stroke_not_applicable: plan.pending_rules.values().any(|pending| pending.stroke)
                && !stroke_is_applicable,
            text_fill_applied: plan.text_fill.is_some() && self.visible_slice_text_count > 0,
            text_fill_not_applicable: plan.text_fill.is_some()
                && self.visible_slice_text_count == 0,
            typography_applied: plan.typography_requested()
                && (self.visible_title_count > 0
                    || self.visible_slice_text_count > 0
                    || self.visible_legend_text_count > 0),
            typography_not_applicable: plan.typography_requested()
                && self.visible_title_count == 0
                && self.visible_slice_text_count == 0
                && self.visible_legend_text_count == 0,
        }
    }
}

fn record_terminal_paint(
    plan: &PieThemePlan,
    expected_paints: &[Option<usize>],
    next_index: &mut usize,
    emitted_index: usize,
    emitted_label: &str,
    emitted_fill: Option<&str>,
) -> (bool, Option<PieSlicePaintOwner>) {
    if emitted_index != *next_index || emitted_index >= expected_paints.len() {
        return (false, None);
    }
    *next_index += 1;
    let Some(paint_index) = expected_paints[emitted_index] else {
        return (false, None);
    };
    let Some(paint) = plan.paints.get(paint_index) else {
        return (false, None);
    };
    let matches = plan.label_indices.get(emitted_label).copied() == Some(paint_index)
        && emitted_fill.is_some_and(|emitted_fill| paint.fill == emitted_fill);
    (matches, matches.then_some(paint.typed_owner).flatten())
}

fn mermaid_owns_stroke_site(effective_config: &MermaidConfig, path: &str) -> bool {
    merman_core::__private::config_path_overrides_typed_default(effective_config, path)
}

fn explicit_config_owns_stroke_site(effective_config: &MermaidConfig, path: &str) -> bool {
    merman_core::__private::explicit_config_owns_path(effective_config, path)
}

fn typed_static_stroke(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    slice_site: bool,
    outer_site: bool,
) -> Option<PieSliceStroke> {
    if !slice_site && !outer_site {
        return None;
    }
    let origin = style.stroke_resolution().winner()?;
    let rule = theme
        .family_rules()
        .find_map(|(index, rule)| (index == origin.rule_index()).then_some(rule))?;
    let facet = FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    if !matches!(rule.variant(), None | Some(ThemeVariant::Default))
        || rule.ordinal().is_some()
        || theme.rule_facet_disposition(origin.rule_index(), facet)
            != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }

    let (css, capability) = match style.stroke_resolution().specified() {
        Specified::Value(CanvasPaint::Transparent) => {
            ("transparent".into(), ThemeCapability::TransparentPaint)
        }
        Specified::Value(CanvasPaint::Solid(color)) => {
            (color.as_css().into_boxed_str(), ThemeCapability::SolidPaint)
        }
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => return None,
    };
    Some(PieSliceStroke {
        css,
        rule_index: origin.rule_index(),
        capability,
        slice_site,
        outer_site,
    })
}

/// Reuses static winners unless ordinal rules require resolution at real Pie occurrences.
fn pie_occurrence_winner_properties(
    theme: &ResolvedDiagramTheme,
    target: ThemeTarget,
    occurrence_count: usize,
    static_style: &ResolvedThemeStyle,
    work_meter: &OperationWorkMeter,
) -> Result<BTreeSet<(usize, ResolvedStyleProperty)>, OperationWorkError> {
    if occurrence_count == 0 {
        return Ok(BTreeSet::new());
    }
    work_meter.charge(theme.family_mechanism_routes().len())?;
    let has_ordinal_rules = theme.family_rules().any(|(_, rule)| {
        rule.target() == target
            && rule.ordinal().is_some()
            && matches!(rule.variant(), None | Some(ThemeVariant::Default))
    });
    if !has_ordinal_rules {
        return Ok(static_style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect());
    }
    let mut winners = BTreeSet::new();
    for ordinal in 1..=occurrence_count {
        work_meter.charge(1)?;
        let style = theme.style_with_work_meter(
            target,
            ThemeVariant::Default,
            Some(ordinal),
            work_meter,
        )?;
        winners.extend(
            style
                .winner_rule_properties()
                .map(|(property, origin)| (origin.rule_index(), property)),
        );
    }
    Ok(winners)
}

#[derive(Debug, Default)]
struct PiePendingEvidence {
    fill: bool,
    stroke: bool,
    text_fill: bool,
}

impl PiePendingEvidence {
    const fn fill_only() -> Self {
        Self {
            fill: true,
            stroke: false,
            text_fill: false,
        }
    }

    const fn text_fill_only() -> Self {
        Self {
            fill: false,
            stroke: false,
            text_fill: true,
        }
    }

    const fn requires_terminal_proof(&self) -> bool {
        self.fill || self.stroke || self.text_fill
    }
}

#[derive(Debug, Default)]
struct PieSliceRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    pending: PiePendingEvidence,
}

#[derive(Debug, Default)]
struct PieTitleRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    fill_pending: bool,
}

#[derive(Debug, Default)]
struct PieTextRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    fill_pending: bool,
}

fn default_pie_palette() -> [&'static str; MERMAID_PIE_PALETTE_SIZE] {
    [
        "#ECECFF",
        "#ffffde",
        "hsl(80, 100%, 56.2745098039%)",
        "hsl(240, 100%, 86.2745098039%)",
        "hsl(60, 100%, 63.5294117647%)",
        "hsl(80, 100%, 76.2745098039%)",
        "hsl(300, 100%, 76.2745098039%)",
        "hsl(180, 100%, 56.2745098039%)",
        "hsl(0, 100%, 56.2745098039%)",
        "hsl(300, 100%, 56.2745098039%)",
        "hsl(150, 100%, 56.2745098039%)",
        "hsl(0, 100%, 66.2745098039%)",
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue, ThemeRule,
        ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;
    use merman_core::diagrams::pie::PieRenderSection;
    use serde_json::json;

    #[test]
    fn bound_css_preserves_raw_values_and_color_fallbacks_without_typed_theme() {
        let plan = PieThemePlan::baseline(
            &PieDiagramRenderModel::default(),
            &json!({"themeVariables": {
                "textColor": "var(--section-color)",
                "taskTextDarkColor": "currentColor",
                "pieStrokeWidth": 3,
                "pieOuterStrokeWidth": "calc(1px + 1em)",
                "pieOpacity": 0.4,
                "pieTitleTextSize": "1.5em"
            }}),
        );
        let css = plan.css_binding();
        assert_eq!(css.title_text_color, "currentColor");
        assert_eq!(css.legend_text_color, "currentColor");
        assert_eq!(css.section_text_color, "var(--section-color)");
        assert_eq!(css.slice_stroke_width, "3");
        assert_eq!(css.outer_stroke_width, "calc(1px + 1em)");
        assert_eq!(css.slice_opacity, "0.4");
        assert_eq!(css.title_text_size, "1.5em");
        assert_eq!(css.title_measurement_style().font_size, 25.0);
        assert_eq!(css.legend_measurement_style().font_size, 17.0);
    }

    #[test]
    fn bound_typed_stroke_keeps_baseline_for_absent_slice_surfaces() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::PieSlice,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#123456").expect("valid stroke")),
                    )),
                ),
            )
            .expect("compile stroke")
            .resolve(DiagramFamilyId::PIE);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let plan = PieThemePlan::resolve(
            &PieDiagramRenderModel::default(),
            &MermaidConfig::from_value(json!({})),
            Some(&theme),
            &meter,
        )
        .expect("resolve stroke");
        let css = plan.css_binding();
        assert_eq!(css.slice_stroke.css(true), "#123456");
        assert_eq!(css.outer_stroke.css(true), "#123456");
        assert_eq!(css.slice_stroke.css(false), "black");
        assert_eq!(css.outer_stroke.css(false), "black");
    }

    fn resolved_slice_palette(colors: &[&str]) -> ResolvedDiagramTheme {
        let palette = OrdinalPalette::new(
            colors
                .iter()
                .map(|color| ThemeColorValue::parse(*color).expect("valid Pie test color")),
        )
        .expect("non-empty Pie test palette");
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::PieSlice, palette),
            ))
            .expect("compile Pie test palette")
            .resolve(DiagramFamilyId::PIE)
    }

    fn palette_plan(colors: &[&str]) -> (PieThemePlan, Vec<String>) {
        let mut model = PieDiagramRenderModel::default();
        model.sections = colors
            .iter()
            .enumerate()
            .map(|(index, _)| PieRenderSection {
                label: format!("Slice {index}"),
                value: 1.0,
            })
            .collect();
        let config = MermaidConfig::from_value(json!({}));
        let theme = resolved_slice_palette(colors);
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let plan = PieThemePlan::resolve(&model, &config, Some(&theme), &work_meter)
            .expect("resolve Pie test palette");
        let labels = model
            .sections
            .iter()
            .map(|section| section.label.clone())
            .collect();
        (plan, labels)
    }

    fn static_fill_plan(fill: CanvasPaint) -> (PieThemePlan, Vec<String>) {
        let mut model = PieDiagramRenderModel::default();
        model.sections = ["Alpha", "Beta"]
            .into_iter()
            .map(|label| PieRenderSection {
                label: label.to_string(),
                value: 1.0,
            })
            .collect();
        let config = MermaidConfig::from_value(json!({}));
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::PieSlice,
                            ThemeStylePatch::default().with_fill(fill),
                        )
                        .for_family(DiagramFamilyId::PIE),
                    ),
                ),
            )
            .expect("compile Pie scalar fill")
            .resolve(DiagramFamilyId::PIE);
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let plan = PieThemePlan::resolve(&model, &config, Some(&theme), &work_meter)
            .expect("resolve Pie scalar fill");
        let labels = model
            .sections
            .iter()
            .map(|section| section.label.clone())
            .collect();
        (plan, labels)
    }

    fn finish_palette_evidence(colors: &[&str]) -> FamilyThemeEvidence {
        let (plan, labels) = palette_plan(colors);
        let mut receipt = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        for (index, label) in labels.iter().enumerate() {
            let fill = plan.fill_for(label).expect("planned Pie slice fill");
            receipt.record_slice(&plan, index, label, Some(fill), Some(PIE_SLICE_CLASS));
            receipt.record_legend(&plan, index, label, Some(fill));
        }
        assert!(plan.record_terminal(receipt));
        plan.finish_evidence()
    }

    #[test]
    fn palette_receipt_rejects_missing_reordered_and_mismatched_terminal_paint() {
        let (plan, labels) = palette_plan(&["#ef4444", "#22c55e"]);

        let mut missing = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        for (index, label) in labels.iter().enumerate() {
            let fill = plan.fill_for(label).expect("planned Pie slice fill");
            missing.record_slice(&plan, index, label, Some(fill), Some(PIE_SLICE_CLASS));
        }
        missing.record_legend(
            &plan,
            0,
            &labels[0],
            Some(plan.fill_for(&labels[0]).expect("first Pie legend fill")),
        );
        assert!(!missing.proves_complete(&plan));

        let mut reordered = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        reordered.record_slice(
            &plan,
            1,
            &labels[1],
            Some(plan.fill_for(&labels[1]).expect("second Pie slice fill")),
            Some(PIE_SLICE_CLASS),
        );
        assert!(!reordered.proves_complete(&plan));

        let mut mismatched = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        mismatched.record_slice(&plan, 0, &labels[0], Some("#000000"), Some(PIE_SLICE_CLASS));
        assert!(!mismatched.proves_complete(&plan));

        let mut missing_fill = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        missing_fill.record_slice(&plan, 0, &labels[0], None, Some(PIE_SLICE_CLASS));
        assert!(!missing_fill.proves_complete(&plan));
    }

    #[test]
    fn scalar_fill_receipt_rejects_missing_or_mismatched_terminal_paint() {
        let (plan, labels) =
            static_fill_plan(CanvasPaint::solid("#111827").expect("valid Pie scalar fill"));

        let mut missing_legend = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie scalar-fill receipt");
        for (index, label) in labels.iter().enumerate() {
            let fill = plan.fill_for(label).expect("planned Pie scalar fill");
            missing_legend.record_slice(&plan, index, label, Some(fill), Some(PIE_SLICE_CLASS));
        }
        missing_legend.record_legend(
            &plan,
            0,
            &labels[0],
            Some(plan.fill_for(&labels[0]).expect("first Pie legend fill")),
        );
        assert!(!missing_legend.proves_complete(&plan));

        let mut mismatched = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie scalar-fill receipt");
        mismatched.record_slice(&plan, 0, &labels[0], Some("#000000"), Some(PIE_SLICE_CLASS));
        assert!(!mismatched.proves_complete(&plan));
    }

    #[test]
    fn transparent_palette_reports_only_transparent_paint() {
        assert_eq!(
            finish_palette_evidence(&["transparent"]).applied_capabilities(),
            BTreeSet::from([ThemeCapability::TransparentPaint])
        );
    }

    #[test]
    fn mixed_palette_reports_transparent_and_solid_paint() {
        assert_eq!(
            finish_palette_evidence(&["transparent", "#22c55e"]).applied_capabilities(),
            BTreeSet::from([
                ThemeCapability::SolidPaint,
                ThemeCapability::TransparentPaint,
            ])
        );
    }
}
