use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::pie::PieDiagramRenderModel;

use crate::config::config_string;
use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const MERMAID_PIE_PALETTE_SIZE: usize = 12;
const PIE_SLICE_CLASS: &str = "pieCircle";
const PIE_OUTER_CIRCLE_CLASS: &str = "pieOuterCircle";
const PIE_SLICE_STROKE_PATH: &str = "themeVariables.pieStrokeColor";
const PIE_OUTER_STROKE_PATH: &str = "themeVariables.pieOuterStrokeColor";
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

#[derive(Debug, Default)]
struct PieTerminalEvidence {
    palette_capabilities: BTreeSet<ThemeCapability>,
    fill_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    stroke_applied: bool,
    stroke_not_applicable: bool,
}

/// Resolves Pie theme surfaces once for layout, SVG emission, and terminal evidence.
#[derive(Debug)]
pub(crate) struct PieThemePlan {
    label_indices: HashMap<String, usize>,
    section_paint_indices: Vec<usize>,
    paints: Vec<PieSlicePaint>,
    stroke: Option<PieSliceStroke>,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    pending_rules: BTreeMap<FamilyThemeMechanismKey, PiePendingEvidence>,
    terminal_evidence: OnceLock<PieTerminalEvidence>,
}

impl PieThemePlan {
    pub(crate) fn baseline(
        model: &PieDiagramRenderModel,
        effective_config: &serde_json::Value,
    ) -> Self {
        let mut plan = Self {
            label_indices: HashMap::new(),
            section_paint_indices: Vec::with_capacity(model.sections.len()),
            paints: Vec::new(),
            stroke: None,
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            pending_rules: BTreeMap::new(),
            terminal_evidence: OnceLock::new(),
        };
        for section in &model.sections {
            let paint_index = plan.insert_mermaid_paint(&section.label, effective_config);
            plan.section_paint_indices.push(paint_index);
        }
        plan
    }

    pub(crate) fn resolve(
        model: &PieDiagramRenderModel,
        effective_config: &MermaidConfig,
        theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(model, effective_config.as_value());
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        plan.resolve_palette(effective_config, theme, work_meter)?;
        plan.resolve_rules(model, effective_config, theme, work_meter)?;
        Ok(plan)
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
        let mut occurrence_fill_winners = BTreeSet::new();
        let mut typed_fill_rules = BTreeSet::new();
        let mut suppressed_fill_rules = BTreeSet::new();
        let needs_occurrence_fill_winners =
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
                        selector: FamilyThemeSelectorShape::Static { variant: None },
                        facet:
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ..
                    } => route.disposition() == FamilyThemeDisposition::TypedAdapter,
                    _ => false,
                });
        if needs_occurrence_fill_winners {
            for (index, paint) in self.paints.iter_mut().enumerate() {
                let style = theme.style_with_work_meter(
                    ThemeTarget::PieSlice,
                    ThemeVariant::Default,
                    Some(index + 1),
                    work_meter,
                )?;
                if let Some(origin) = style.fill_resolution().winner() {
                    occurrence_fill_winners.insert(origin.rule_index());
                }
                let Some(fill) = resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::PieSlice],
                    DirectStaticSelectorDomain::Unqualified,
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
        let winner_properties = style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        self.stroke = typed_static_stroke(theme, &style, slice_site, outer_site);

        let occurrence_count = model.sections.len();
        // Pie only emits a shared static stroke rule today. Ordinal stroke winners therefore
        // remain residual, but they must still be observed at the occurrences they match.
        let has_ordinal_stroke_routes =
            theme
                .family_mechanism_routes()
                .iter()
                .copied()
                .any(|route| {
                    matches!(
                        route.mechanism(),
                        FamilyThemeMechanism::RuleFacet {
                            target: ThemeTarget::PieSlice,
                            selector: FamilyThemeSelectorShape::Ordinal { .. },
                            facet: FamilyThemeRuleFacet::Stroke(_),
                            ..
                        }
                    )
                });
        let mut occurrence_stroke_winners = BTreeSet::new();
        if has_ordinal_stroke_routes {
            for ordinal in 1..=occurrence_count {
                let occurrence_style = theme.style_with_work_meter(
                    ThemeTarget::PieSlice,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                if let Some(origin) = occurrence_style.stroke_resolution().winner() {
                    occurrence_stroke_winners.insert(origin.rule_index());
                }
            }
        }
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
            let route_won = winner_properties
                .contains(&(rule_index, resolved_style_property_for_facet(facet)))
                || (matches!(facet, FamilyThemeRuleFacet::Fill(_))
                    && occurrence_fill_winners.contains(&rule_index))
                || (matches!(facet, FamilyThemeRuleFacet::Stroke(_))
                    && occurrence_stroke_winners.contains(&rule_index));
            let qualified_variant = matches!(
                selector,
                FamilyThemeSelectorShape::Static { variant: Some(_) }
                    | FamilyThemeSelectorShape::Ordinal {
                        variant: Some(_),
                        ..
                    }
            );
            if !route_won && !qualified_variant {
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
                    FamilyThemeSelectorShape::Static { variant: None },
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
                    FamilyThemeSelectorShape::Static { variant: None },
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

    pub(crate) fn begin_terminal_receipt<'a>(
        &self,
        visible_slice_labels: impl IntoIterator<Item = &'a str>,
    ) -> Option<PieThemeReceipt> {
        (self.palette_key.is_some() || !self.pending_rules.is_empty() || self.stroke.is_some())
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
                let FamilyThemeMechanismKey::Rule { index, .. } = key else {
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
    values_match: bool,
    slice_classes_match: bool,
    outer_circle_class: Option<Box<str>>,
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
            values_match,
            slice_classes_match: true,
            outer_circle_class: None,
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

    fn proves_complete(&self, plan: &PieThemePlan) -> bool {
        let stroke_is_applicable = plan.stroke.is_some() && !self.expected_slice_paints.is_empty();
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
    }

    fn into_terminal_evidence(self, plan: &PieThemePlan) -> PieTerminalEvidence {
        let stroke_is_applicable = plan.stroke.is_some() && !self.expected_slice_paints.is_empty();
        PieTerminalEvidence {
            palette_capabilities: self.palette_capabilities,
            fill_capabilities: self.fill_capabilities,
            stroke_applied: plan.pending_rules.values().any(|pending| pending.stroke)
                && stroke_is_applicable,
            stroke_not_applicable: plan.pending_rules.values().any(|pending| pending.stroke)
                && !stroke_is_applicable,
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
    if rule.variant().is_some()
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

#[derive(Debug, Default)]
struct PiePendingEvidence {
    fill: bool,
    stroke: bool,
}

impl PiePendingEvidence {
    const fn requires_terminal_proof(&self) -> bool {
        self.fill || self.stroke
    }
}

#[derive(Debug, Default)]
struct PieSliceRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    pending: PiePendingEvidence,
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
