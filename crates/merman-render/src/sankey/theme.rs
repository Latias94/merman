use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedStyleProperty, Specified, ThemeCapability,
    ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence,
    FamilyThemeOutcomes, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::SankeyDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::SankeyConfigView;

const TABLEAU10: [&str; 10] = [
    "#4e79a7", "#f28e2c", "#e15759", "#76b7b2", "#59a14f", "#edc949", "#af7aa1", "#ff9da7",
    "#9c755f", "#bab0ab",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SankeyLabelSurface {
    Plain,
    OutlineBackground,
    OutlineForeground,
}

impl SankeyLabelSurface {
    pub(crate) const fn class_name(self) -> Option<&'static str> {
        match self {
            Self::Plain => None,
            Self::OutlineBackground => Some("sankey-label-bg"),
            Self::OutlineForeground => Some("sankey-label-fg"),
        }
    }
}

/// Final inherited Sankey font stack shared by base CSS, label CSS, and terminal evidence.
#[derive(Debug)]
pub(crate) struct SankeyTypographyThemePlan {
    common_css: crate::svg::PreparedCommonCss,
    label_background: String,
    link_color: String,
    outlined_labels: bool,
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeOutcomes,
    text_fill: Option<DirectStaticPaint>,
    text_rules: BTreeMap<usize, Vec<SankeyTextRuleObservation>>,
    text_rules_have_ordinals: bool,
    config_owns_text_fill: bool,
    terminal_receipt: OnceLock<SankeyTypographyTerminalSeal>,
}

impl SankeyTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        label_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let config_owns_text_fill =
            effective_config.config_paint_overrides_typed_default("themeVariables.textColor");
        let mut text_fill = None;
        let mut text_rules = BTreeMap::<usize, Vec<SankeyTextRuleObservation>>::new();
        let mut has_ordinals = false;
        if let Some(theme) = theme {
            let static_style = theme.style_with_work_meter(
                ThemeTarget::Text,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            text_fill = resolve_direct_static_fill(
                theme,
                &static_style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            );
            work_meter.charge(theme.family_mechanism_routes().len())?;
            let mut facets = BTreeMap::new();
            for route in theme.family_mechanism_routes() {
                if let FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Text,
                    selector,
                    facet,
                } = route.mechanism()
                {
                    text_rules.entry(rule_index).or_default();
                    has_ordinals |= matches!(
                        selector,
                        crate::diagram_theme::FamilyThemeSelectorShape::Ordinal { .. }
                    );
                    facets.insert(
                        (rule_index, resolved_style_property_for_facet(facet)),
                        facet,
                    );
                }
            }
            let occurrence_count = if has_ordinals {
                label_count
            } else {
                usize::from(label_count != 0)
            };
            for ordinal in 1..=occurrence_count {
                work_meter.charge(1)?;
                let ordinal_style;
                let style = if has_ordinals {
                    ordinal_style = theme.style_with_work_meter(
                        ThemeTarget::Text,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    &ordinal_style
                } else {
                    &static_style
                };
                for (property, origin) in style.winner_rule_properties() {
                    if config_owns_text_fill && property == ResolvedStyleProperty::Fill {
                        continue;
                    }
                    let Some(facet) = facets.get(&(origin.rule_index(), property)) else {
                        continue;
                    };
                    let observations = text_rules
                        .get_mut(&origin.rule_index())
                        .expect("registered text rule");
                    let ordinal = has_ordinals.then_some(ordinal);
                    if observations
                        .last()
                        .is_none_or(|observation| observation.ordinal != ordinal)
                    {
                        observations.push(SankeyTextRuleObservation {
                            ordinal,
                            ..Default::default()
                        });
                    }
                    let observation = observations.last_mut().expect("observed text rule");
                    if property == ResolvedStyleProperty::Fill
                        && text_fill
                            .as_ref()
                            .is_some_and(|fill| fill.rule_index() == origin.rule_index())
                    {
                        observation.fill_pending = true;
                    } else {
                        observation
                            .residual
                            .get_or_insert(unsupported_residual_for_facet(*facet));
                    }
                }
            }
        }
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let mut common_css = crate::svg::PreparedCommonCss::new(
            effective_config.as_value(),
            Some(inherited_font_stack.font_family_css()),
        );
        if !config_owns_text_fill && let Some(fill) = &text_fill {
            common_css = common_css.with_text_color(fill.css());
        }
        let config_view = SankeyConfigView::new(effective_config.as_value());
        Ok(Self {
            common_css,
            label_background: crate::config::config_string(
                effective_config.as_value(),
                &["themeVariables", "mainBkg"],
            )
            .or_else(|| {
                crate::config::config_string(
                    effective_config.as_value(),
                    &["themeVariables", "background"],
                )
            })
            .unwrap_or_else(|| "#fff".to_owned()),
            link_color: config_view.link_color(),
            outlined_labels: config_view.outlined_labels(),
            inherited_font_stack,
            evidence: FamilyThemeOutcomes::default(),
            text_fill,
            text_rules,
            text_rules_have_ordinals: has_ordinals,
            config_owns_text_fill,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.common_css.font_family()
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }
    pub(crate) fn label_background(&self) -> &str {
        &self.label_background
    }
    pub(crate) fn link_color(&self) -> &str {
        &self.link_color
    }
    pub(crate) fn outlined_labels(&self) -> bool {
        self.outlined_labels
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        label_count: usize,
        outlined_labels: bool,
    ) -> Option<SankeyTypographyThemeReceipt<'_>> {
        (self.inherited_font_stack.typography_requested() || !self.text_rules.is_empty())
            .then(|| SankeyTypographyThemeReceipt::new(self, label_count, outlined_labels))
    }

    pub(crate) fn record_terminal(&self, receipt: SankeyTypographyThemeReceipt<'_>) -> bool {
        receipt
            .seal()
            .is_some_and(|seal| self.terminal_receipt.set(seal).is_ok())
    }

    pub(crate) fn text_fill_css(&self) -> Option<&str> {
        (!self.config_owns_text_fill)
            .then_some(self.text_fill.as_ref())
            .flatten()
            .map(DirectStaticPaint::css)
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        let Some(receipt) = self.terminal_receipt.get() else {
            if self.inherited_font_stack.typed_font_stack_requested() {
                self.inherited_font_stack
                    .record_unsupported_typography_evidence(evidence, true);
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
            for (index, observations) in &self.text_rules {
                let key = FamilyThemeMechanismKey::Rule {
                    index: *index,
                    target: ThemeTarget::Text,
                };
                if let Some(reason) = observations
                    .iter()
                    .find_map(|observation| observation.residual)
                {
                    evidence.mark_residual(key, reason);
                } else if observations
                    .iter()
                    .any(|observation| observation.fill_pending)
                {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
            return;
        };
        self.inherited_font_stack
            .record_unsupported_typography_evidence(evidence, receipt.has_visible_label);
        if self.inherited_font_stack.typed_font_stack_requested() {
            let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
            if !receipt.has_visible_label {
                evidence.mark_not_applicable(key);
            } else if self.inherited_font_stack.typed_font_stack_active() {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                match self.inherited_font_stack.outcome() {
                    InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                    InheritedFontStackOutcome::Typed | InheritedFontStackOutcome::Unsupported => {
                        evidence
                            .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography)
                    }
                    InheritedFontStackOutcome::Inactive => {}
                }
            }
        }
        for (index, observations) in &self.text_rules {
            let mut observation = SankeyTextRuleObservation::default();
            for occurrence in observations {
                if occurrence.ordinal.is_some_and(|ordinal| {
                    receipt
                        .visible_labels
                        .as_ref()
                        .is_none_or(|visible| visible.get(ordinal - 1).copied() != Some(true))
                }) {
                    continue;
                }
                observation.fill_pending |= occurrence.fill_pending;
                observation.residual = observation.residual.or(occurrence.residual);
            }
            let key = FamilyThemeMechanismKey::Rule {
                index: *index,
                target: ThemeTarget::Text,
            };
            if !receipt.has_visible_label {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.fill_pending {
                if let Some(fill) = self.text_fill.as_ref()
                    && receipt.text_fill_css.as_deref() == Some(fill.css())
                {
                    evidence.mark_applied_with_capabilities(key, [fill.capability()]);
                } else {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
                }
            } else {
                evidence.mark_not_applicable(key);
            }
        }
    }
}

#[derive(Debug, Default)]
struct SankeyTextRuleObservation {
    ordinal: Option<usize>,
    fill_pending: bool,
    residual: Option<FamilyThemeResidualReason>,
}

/// Milestone issued only after the final Sankey stylesheet writer completes successfully.
#[derive(Debug)]
pub(crate) struct SankeyTypographyCssEmission<'a> {
    font_family_css: &'a str,
    all_font_surfaces_match: bool,
    text_fill_css: &'a str,
    label_foreground_fill_css: &'a str,
}

impl<'a> SankeyTypographyCssEmission<'a> {
    pub(crate) fn from_successful_writes(
        font_family_css: &'a str,
        all_font_surfaces_match: bool,
        text_fill_css: &'a str,
        label_foreground_fill_css: &'a str,
    ) -> Self {
        Self {
            font_family_css,
            all_font_surfaces_match,
            text_fill_css,
            label_foreground_fill_css,
        }
    }
}

/// Writer-owned proof that the stylesheet and expected label passes reached the final SVG sink.
#[derive(Debug)]
pub(crate) struct SankeyTypographyThemeReceipt<'a> {
    expected_labels_per_pass: usize,
    outlined_labels: bool,
    expected_font_family_css: &'a str,
    expected_text_fill_css: Option<&'a str>,
    css_emitted: bool,
    next_pass: usize,
    labels_in_pass: usize,
    has_visible_label: bool,
    visible_labels: Option<Vec<bool>>,
    valid: bool,
}

#[derive(Debug)]
struct SankeyTypographyTerminalSeal {
    has_visible_label: bool,
    visible_labels: Option<Vec<bool>>,
    text_fill_css: Option<String>,
}

impl<'a> SankeyTypographyThemeReceipt<'a> {
    fn new(
        plan: &'a SankeyTypographyThemePlan,
        expected_labels_per_pass: usize,
        outlined_labels: bool,
    ) -> Self {
        let expected_pass_count = if outlined_labels { 2 } else { 1 };
        Self {
            expected_labels_per_pass,
            outlined_labels,
            expected_font_family_css: plan.font_family_css(),
            expected_text_fill_css: plan.text_fill_css(),
            css_emitted: false,
            next_pass: if expected_labels_per_pass == 0 {
                expected_pass_count
            } else {
                0
            },
            labels_in_pass: 0,
            has_visible_label: false,
            visible_labels: plan
                .text_rules_have_ordinals
                .then(|| Vec::with_capacity(expected_labels_per_pass)),
            valid: true,
        }
    }

    pub(crate) fn record_css_emission(&mut self, emission: SankeyTypographyCssEmission<'_>) {
        self.valid &= !self.css_emitted
            && emission.all_font_surfaces_match
            && emission.font_family_css == self.expected_font_family_css
            && self
                .expected_text_fill_css()
                .is_none_or(|expected| emission.text_fill_css == expected)
            && self
                .expected_text_fill_css()
                .is_none_or(|expected| emission.label_foreground_fill_css == expected);
        self.css_emitted = true;
    }

    pub(crate) fn record_label(&mut self, surface: SankeyLabelSurface, visible: bool) {
        if self.next_pass >= self.expected_pass_count() || self.expected_labels_per_pass == 0 {
            self.valid = false;
            return;
        }
        self.valid &= surface == self.expected_surface();
        if let Some(visible_labels) = &mut self.visible_labels {
            if self.next_pass == 0 {
                visible_labels.push(visible);
            } else {
                self.valid &= visible_labels.get(self.labels_in_pass).copied() == Some(visible);
            }
        }
        self.has_visible_label |= visible;
        self.labels_in_pass = self.labels_in_pass.saturating_add(1);
        if self.labels_in_pass == self.expected_labels_per_pass {
            self.labels_in_pass = 0;
            self.next_pass = self.next_pass.saturating_add(1);
        } else if self.labels_in_pass > self.expected_labels_per_pass {
            self.valid = false;
        }
    }

    fn expected_surface(&self) -> SankeyLabelSurface {
        if !self.outlined_labels {
            return SankeyLabelSurface::Plain;
        }
        if self.next_pass == 0 {
            SankeyLabelSurface::OutlineBackground
        } else {
            SankeyLabelSurface::OutlineForeground
        }
    }

    fn expected_pass_count(&self) -> usize {
        if self.outlined_labels { 2 } else { 1 }
    }

    fn seal(self) -> Option<SankeyTypographyTerminalSeal> {
        (self.css_emitted
            && self.valid
            && self.next_pass == self.expected_pass_count()
            && self.labels_in_pass == 0
            && self
                .visible_labels
                .as_ref()
                .is_none_or(|visible| visible.len() == self.expected_labels_per_pass))
        .then_some(SankeyTypographyTerminalSeal {
            has_visible_label: self.has_visible_label,
            visible_labels: self.visible_labels,
            text_fill_css: self.expected_text_fill_css.map(str::to_owned),
        })
    }

    fn expected_text_fill_css(&self) -> Option<&str> {
        self.expected_text_fill_css
    }
}

#[derive(Debug)]
struct SankeyNodePaint {
    id: String,
    ordinal: usize,
    fill_css: String,
    source_owned: bool,
    typed_capability: Option<ThemeCapability>,
    stroke: Option<DirectPaintExpectation>,
}

#[derive(Debug, Clone, Copy)]
struct SankeyNodeLookup {
    first_layout_index: usize,
    last_paint_index: usize,
}

#[derive(Debug)]
struct SankeyPreparedLink {
    source_layout_index: usize,
    target_layout_index: usize,
    paint: SankeyLinkPaintSelection,
}

#[derive(Debug, Clone, Copy)]
enum SankeyLinkPaintSelection {
    NodeFill {
        paint_index: usize,
    },
    Gradient {
        source_paint_index: usize,
        target_paint_index: usize,
    },
    Literal,
}

pub(crate) enum SankeyLinkPaint<'a> {
    Solid(&'a str),
    Gradient { source: &'a str, target: &'a str },
}

pub(crate) struct SankeyLinkView<'a> {
    pub(crate) source_layout_index: usize,
    pub(crate) target_layout_index: usize,
    pub(crate) paint: SankeyLinkPaint<'a>,
}

/// Resolves Sankey node paint once for terminal rectangles, derived link colors, and evidence.
#[derive(Debug)]
pub(crate) struct SankeyNodePalettePlan {
    node_indices: HashMap<String, SankeyNodeLookup>,
    links: Option<Vec<SankeyPreparedLink>>,
    literal_link_paint: Box<str>,
    #[cfg(test)]
    endpoint_lookups: usize,
    paints: Vec<SankeyNodePaint>,
    evidence: FamilyThemeOutcomes,
    palette_key: Option<FamilyThemeMechanismKey>,
    pending_rules: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    terminal_capabilities: OnceLock<BTreeSet<ThemeCapability>>,
}

impl SankeyNodePalettePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        layout: &SankeyDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(effective_config.as_value(), layout);
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeOutcomes::default();
        work_meter.charge(plan.paints.len())?;
        let source_owned_fill = plan
            .paints
            .iter()
            .map(|paint| paint.source_owned)
            .collect::<Vec<_>>();
        let mut observations = BTreeMap::<usize, SankeyNodeRuleObservation>::new();
        let mut facets = BTreeMap::new();
        for route in theme.family_mechanism_routes() {
            if let FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Node,
                rule_index,
                facet,
                ..
            } = route.mechanism()
            {
                facets.insert(
                    (rule_index, resolved_style_property_for_facet(facet)),
                    facet,
                );
                observations.entry(rule_index).or_default();
            }
        }
        for paint in &mut plan.paints {
            let style = theme.style_with_work_meter(
                ThemeTarget::Node,
                ThemeVariant::Default,
                Some(paint.ordinal),
                work_meter,
            )?;
            let fill = (!paint.source_owned)
                .then(|| {
                    sankey_node_paint(
                        theme,
                        style.fill_resolution(),
                        FamilyThemeRuleFacet::fill,
                        &paint.fill_css,
                    )
                })
                .flatten();
            if let Some(fill) = &fill {
                paint.fill_css = fill.css().to_owned();
            }
            paint.stroke = sankey_node_paint(
                theme,
                style.stroke_resolution(),
                FamilyThemeRuleFacet::stroke,
                "none",
            );
            work_meter.charge(facets.len())?;
            for (property, origin) in style.winner_rule_properties() {
                if property == ResolvedStyleProperty::Fill && paint.source_owned {
                    continue;
                }
                let Some(facet) = facets.get(&(origin.rule_index(), property)) else {
                    continue;
                };
                let observation = observations
                    .get_mut(&origin.rule_index())
                    .expect("registered Sankey node rule");
                let direct = match property {
                    ResolvedStyleProperty::Fill => fill.as_ref(),
                    ResolvedStyleProperty::Stroke => paint.stroke.as_ref(),
                    _ => None,
                };
                if let Some(direct) = direct
                    && direct.rule_index() == origin.rule_index()
                {
                    observation.capabilities.insert(direct.capability());
                } else {
                    observation
                        .residual
                        .get_or_insert(unsupported_residual_for_facet(*facet));
                }
            }
        }
        for (index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Node,
            };
            if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if observation.capabilities.is_empty() {
                plan.evidence.mark_not_applicable(key);
            } else {
                plan.pending_rules.insert(index, observation.capabilities);
            }
        }
        let render_settings = SankeyConfigView::new(effective_config.as_value()).render_settings();
        let label_occurrences = layout
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| render_settings.show_values || !node.id.trim().is_empty())
            .map(|(index, _)| (index + 1, ThemeVariant::Default))
            .collect::<Vec<_>>();
        let source_owned_text_fill =
            vec![
                effective_config.config_paint_overrides_typed_default("themeVariables.textColor");
                layout.nodes.len()
            ];
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[
                // Sankey emits node labels, but no visual title, including frontmatter titles.
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Title,
                    TerminalVariantDomain::uniform(0, ThemeVariant::Default),
                ),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Node,
                    TerminalVariantDomain::uniform(plan.paints.len(), ThemeVariant::Default),
                )
                // Authored nodeColors owns fill only, independently of node stroke.
                .with_source_owned_fill(&source_owned_fill),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Text,
                    TerminalVariantDomain::indexed(&label_occurrences),
                )
                .with_source_owned_fill(&source_owned_text_fill),
            ],
            work_meter,
        )?;
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::Node) else {
            return Ok(plan);
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        };

        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                plan.palette_key = Some(key.clone());
                let mut missing_typed_color = false;
                for paint in &mut plan.paints {
                    if paint.source_owned {
                        continue;
                    }
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Node,
                        ThemeVariant::Default,
                        Some(paint.ordinal),
                        work_meter,
                    )?;
                    if style.fill_resolution().winner().is_some() {
                        continue;
                    }
                    let Some(color) = theme.series_color(ThemeTarget::Node, paint.ordinal) else {
                        missing_typed_color = true;
                        continue;
                    };
                    paint.fill_css = color.as_css();
                    paint.typed_capability = Some(if color.is_transparent() {
                        ThemeCapability::TransparentPaint
                    } else {
                        ThemeCapability::SolidPaint
                    });
                }
                if missing_typed_color {
                    plan.evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
                    plan.palette_key = None;
                }
            }
            FamilyThemeDisposition::Unsupported => plan
                .evidence
                .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
        }

        Ok(plan)
    }

    fn baseline(effective_config: &serde_json::Value, layout: &SankeyDiagramLayout) -> Self {
        let render_settings = SankeyConfigView::new(effective_config).render_settings();
        let node_colors = render_settings.node_colors;
        let mut node_indices = HashMap::with_capacity(layout.nodes.len());
        let mut paints = Vec::with_capacity(layout.nodes.len());
        let mut default_color_domain = HashMap::with_capacity(layout.nodes.len());

        for (node_index, node) in layout.nodes.iter().enumerate() {
            let explicit_fill = node_colors
                .and_then(|colors| colors.get(&node.id))
                .and_then(serde_json::Value::as_str);
            node_indices
                .entry(node.id.clone())
                .and_modify(|lookup: &mut SankeyNodeLookup| lookup.last_paint_index = node_index)
                .or_insert(SankeyNodeLookup {
                    first_layout_index: node_index,
                    last_paint_index: node_index,
                });
            let fill = explicit_fill.unwrap_or_else(|| {
                let next_index = default_color_domain.len();
                let palette_index = *default_color_domain
                    .entry(node.id.as_str())
                    .or_insert(next_index);
                TABLEAU10[palette_index % TABLEAU10.len()]
            });
            paints.push(SankeyNodePaint {
                id: node.id.clone(),
                ordinal: node_index + 1,
                fill_css: fill.to_string(),
                source_owned: explicit_fill.is_some(),
                typed_capability: None,
                stroke: None,
            });
        }

        Self {
            node_indices,
            links: None,
            literal_link_paint: "".into(),
            #[cfg(test)]
            endpoint_lookups: 0,
            paints,
            evidence: FamilyThemeOutcomes::default(),
            palette_key: None,
            pending_rules: BTreeMap::new(),
            terminal_capabilities: OnceLock::new(),
        }
    }

    pub(crate) fn fill_for(&self, node_index: usize, node_id: &str) -> Option<&str> {
        self.paints
            .get(node_index)
            .filter(|paint| paint.id == node_id)
            .map(|paint| paint.fill_css.as_str())
    }

    pub(crate) fn stroke_for(&self, node_index: usize, node_id: &str) -> Option<&str> {
        self.paints
            .get(node_index)
            .filter(|paint| paint.id == node_id)
            .and_then(|paint| paint.stroke.as_ref())
            .map(DirectPaintExpectation::css)
    }

    pub(crate) fn stroke_outset(&self, node_index: usize) -> f64 {
        // The rectangle writer retains SVG's default 1px stroke width. Clear and
        // fully transparent paint do not expand the painted-content bounds.
        if self
            .paints
            .get(node_index)
            .and_then(|paint| paint.stroke.as_ref())
            .is_some_and(|stroke| stroke.capability() == ThemeCapability::SolidPaint)
        {
            0.5
        } else {
            0.0
        }
    }

    /// Selects each link once from final node paints without allocating terminal SVG IDs.
    pub(crate) fn prepare_links(
        &mut self,
        layout: &SankeyDiagramLayout,
        link_color: &str,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<()> {
        assert!(
            self.links.is_none(),
            "Sankey links prepared once before emission"
        );
        enum Mode {
            Source,
            Target,
            Gradient,
            Literal,
        }
        let mode = match link_color {
            "source" => Mode::Source,
            "target" => Mode::Target,
            "gradient" => Mode::Gradient,
            _ => Mode::Literal,
        };
        let mut links = Vec::with_capacity(layout.links.len());
        for link in &layout.links {
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            #[cfg(test)]
            {
                self.endpoint_lookups += 1;
            }
            let source =
                self.node_indices
                    .get(&link.source)
                    .ok_or_else(|| crate::Error::InvalidModel {
                        message: format!("missing source node {}", link.source),
                    })?;
            #[cfg(test)]
            {
                self.endpoint_lookups += 1;
            }
            let target =
                self.node_indices
                    .get(&link.target)
                    .ok_or_else(|| crate::Error::InvalidModel {
                        message: format!("missing target node {}", link.target),
                    })?;
            let paint = match mode {
                Mode::Source => SankeyLinkPaintSelection::NodeFill {
                    paint_index: source.last_paint_index,
                },
                Mode::Target => SankeyLinkPaintSelection::NodeFill {
                    paint_index: target.last_paint_index,
                },
                Mode::Gradient => SankeyLinkPaintSelection::Gradient {
                    source_paint_index: source.last_paint_index,
                    target_paint_index: target.last_paint_index,
                },
                Mode::Literal => SankeyLinkPaintSelection::Literal,
            };
            links.push(SankeyPreparedLink {
                source_layout_index: source.first_layout_index,
                target_layout_index: target.first_layout_index,
                paint,
            });
        }
        self.literal_link_paint = link_color.into();
        self.links = Some(links);
        Ok(())
    }

    pub(crate) fn terminal_link(&self, link_index: usize) -> Option<SankeyLinkView<'_>> {
        let link = self.links.as_ref()?.get(link_index)?;
        let paint = match link.paint {
            SankeyLinkPaintSelection::NodeFill { paint_index } => {
                SankeyLinkPaint::Solid(&self.paints.get(paint_index)?.fill_css)
            }
            SankeyLinkPaintSelection::Gradient {
                source_paint_index,
                target_paint_index,
            } => SankeyLinkPaint::Gradient {
                source: &self.paints.get(source_paint_index)?.fill_css,
                target: &self.paints.get(target_paint_index)?.fill_css,
            },
            SankeyLinkPaintSelection::Literal => SankeyLinkPaint::Solid(&self.literal_link_paint),
        };
        Some(SankeyLinkView {
            source_layout_index: link.source_layout_index,
            target_layout_index: link.target_layout_index,
            paint,
        })
    }

    #[cfg(test)]
    pub(crate) fn link_lookup_stats(&self) -> (usize, usize) {
        (self.paints.len(), self.endpoint_lookups)
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<SankeyNodePaletteReceipt> {
        (self.palette_key.is_some() || !self.pending_rules.is_empty())
            .then(|| SankeyNodePaletteReceipt::new(self.paints.len()))
    }

    pub(crate) fn record_terminal(&self, receipt: SankeyNodePaletteReceipt) -> bool {
        if (self.palette_key.is_none() && self.pending_rules.is_empty())
            || !receipt.proves_complete(&self.paints)
        {
            return false;
        }
        self.terminal_capabilities
            .set(receipt.applied_capabilities)
            .is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        for (index, capabilities) in &self.pending_rules {
            let key = FamilyThemeMechanismKey::Rule {
                index: *index,
                target: ThemeTarget::Node,
            };
            if self.terminal_capabilities.get().is_some() {
                evidence.mark_applied_with_capabilities(key, capabilities.iter().copied());
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
        let Some(key) = self.palette_key.clone() else {
            return;
        };
        match self.terminal_capabilities.get() {
            Some(capabilities) if capabilities.is_empty() => evidence.mark_not_applicable(key),
            Some(capabilities) => {
                evidence.mark_applied_with_capabilities(key, capabilities.iter().copied())
            }
            None => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette)
            }
        }
    }
}

#[derive(Default)]
struct SankeyNodeRuleObservation {
    capabilities: BTreeSet<ThemeCapability>,
    residual: Option<FamilyThemeResidualReason>,
}

fn sankey_node_paint(
    theme: &ResolvedDiagramTheme,
    resolution: &crate::diagram_theme::ResolvedProperty<CanvasPaint>,
    facet: fn(&Specified<CanvasPaint>) -> Option<FamilyThemeRuleFacet>,
    baseline: &str,
) -> Option<DirectPaintExpectation> {
    let origin = resolution.winner()?;
    if origin.target() != ThemeTarget::Node
        || origin.ordinal().is_some()
        || !matches!(origin.variant(), None | Some(ThemeVariant::Default))
        || theme.rule_facet_disposition(origin.rule_index(), facet(resolution.specified())?)
            != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    let (css, capability) = match resolution.specified() {
        // Clear restores the original categorical fill or the rectangle's absent stroke.
        Specified::Clear => (baseline.to_owned(), ThemeCapability::SemanticRules),
        Specified::Value(CanvasPaint::Transparent) => {
            ("transparent".to_owned(), ThemeCapability::TransparentPaint)
        }
        Specified::Value(CanvasPaint::Solid(color)) => (
            color.as_css(),
            if color.is_transparent() {
                ThemeCapability::TransparentPaint
            } else {
                ThemeCapability::SolidPaint
            },
        ),
        _ => return None,
    };
    Some(DirectPaintExpectation::new(
        origin.rule_index(),
        css,
        capability,
    ))
}

/// Writer-owned proof that every Sankey node emitted its planned rectangle fill and stroke.
#[derive(Debug)]
pub(crate) struct SankeyNodePaletteReceipt {
    expected_count: usize,
    next_index: usize,
    values_match: bool,
    applied_capabilities: BTreeSet<ThemeCapability>,
}

impl SankeyNodePaletteReceipt {
    fn new(expected_count: usize) -> Self {
        Self {
            expected_count,
            next_index: 0,
            values_match: true,
            applied_capabilities: BTreeSet::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn record_node(
        &mut self,
        plan: &SankeyNodePalettePlan,
        emitted_index: usize,
        emitted_id: &str,
        emitted_ordinal: usize,
        emitted_fill: Option<&str>,
    ) {
        self.record_node_paints(
            plan,
            emitted_index,
            emitted_id,
            emitted_ordinal,
            (emitted_fill, None),
        );
    }

    pub(crate) fn record_node_paints(
        &mut self,
        plan: &SankeyNodePalettePlan,
        emitted_index: usize,
        emitted_id: &str,
        emitted_ordinal: usize,
        emitted_paints: (Option<&str>, Option<&str>),
    ) {
        if emitted_index != self.next_index {
            self.values_match = false;
            return;
        }
        self.next_index = self.next_index.saturating_add(1);
        let Some(paint) = plan.paints.get(emitted_index) else {
            self.values_match = false;
            return;
        };
        let matches = paint.id == emitted_id
            && paint.ordinal == emitted_ordinal
            && emitted_paints.0.is_some_and(|fill| fill == paint.fill_css)
            && emitted_paints.1 == paint.stroke.as_ref().map(DirectPaintExpectation::css);
        self.values_match &= matches;
        if matches {
            self.applied_capabilities.extend(paint.typed_capability);
        }
    }

    fn proves_complete(&self, paints: &[SankeyNodePaint]) -> bool {
        self.values_match
            && self.expected_count == paints.len()
            && self.next_index == self.expected_count
    }
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
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette, ThemeColorValue,
        ThemeRuleSet, ThemeTextStyle, TypographySpec,
    };
    use crate::model::SankeyNodeLayout;
    use crate::resources::RenderResourcePolicy;
    use serde_json::json;

    #[test]
    fn unthemed_binding_keeps_label_background_and_link_paint_semantics() {
        let config = crate::test_support::presentation_config(json!({
            "sankey": {"linkColor": "var(--link)", "labelStyle": "outlined"},
            "themeVariables": {"background": "fallback", "mainBkg": "",
                "textColor": "currentColor"}
        }));
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let plan = SankeyTypographyThemePlan::resolve(None, &config, 1, &meter).unwrap();
        assert_eq!(plan.label_background(), "");
        assert_eq!(plan.link_color(), "var(--link)");
        assert!(plan.outlined_labels());
        assert_eq!(plan.common_css().text_color(), "currentColor");

        let ignored = crate::test_support::presentation_config(json!({"sankey": {
            "$ref": "schema", "linkColor": "source", "labelStyle": "outlined"
        }}));
        let fallback = SankeyTypographyThemePlan::resolve(None, &ignored, 1, &meter).unwrap();
        assert_eq!(fallback.link_color(), "gradient");
        assert!(!fallback.outlined_labels());
        assert_eq!(fallback.label_background(), "#fff");
    }

    fn layout(ids: &[&str]) -> SankeyDiagramLayout {
        SankeyDiagramLayout {
            bounds: None,
            width: 600.0,
            height: 400.0,
            node_width: 10.0,
            node_padding: 12.0,
            nodes: ids
                .iter()
                .enumerate()
                .map(|(index, id)| SankeyNodeLayout {
                    id: (*id).to_string(),
                    index,
                    depth: index,
                    height: ids.len().saturating_sub(index + 1),
                    layer: index,
                    value: 1.0,
                    x0: index as f64 * 20.0,
                    x1: index as f64 * 20.0 + 10.0,
                    y0: 0.0,
                    y1: 10.0,
                })
                .collect(),
            links: Vec::new(),
        }
    }

    fn resolved_node_palette(colors: &[&str]) -> ResolvedDiagramTheme {
        let palette = OrdinalPalette::new(
            colors
                .iter()
                .map(|color| ThemeColorValue::parse(color).expect("valid Sankey test color")),
        )
        .expect("non-empty Sankey test palette");
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette),
            ))
            .expect("compile Sankey test palette")
            .resolve(DiagramFamilyId::SANKEY)
    }

    fn resolved_typography(typography: ThemeTextStyle) -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::SANKEY, typography),
            ))
            .expect("compile Sankey test typography")
            .resolve(DiagramFamilyId::SANKEY)
    }

    #[test]
    fn scalar_node_requests_are_not_applicable_without_terminal_nodes() {
        use crate::diagram_theme::{CanvasPaint, ThemeRule, ThemeStylePatch};

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
            .resolve(DiagramFamilyId::SANKEY);
        let config = crate::test_support::presentation_config(json!({}));
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let plan =
            SankeyNodePalettePlan::resolve(Some(&theme), &config, &layout(&[]), &meter).unwrap();
        let evidence = collect_evidence(&theme, |evidence| plan.record_evidence(evidence));
        assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn scalar_node_stroke_receipt_rejects_missing_or_wrong_terminal_stroke() {
        let mut plan = SankeyNodePalettePlan::baseline(&json!({}), &layout(&["A"]));
        plan.paints[0].stroke = Some(DirectPaintExpectation::new(
            0,
            "#123456",
            ThemeCapability::SolidPaint,
        ));
        plan.pending_rules
            .insert(0, BTreeSet::from([ThemeCapability::SolidPaint]));
        let fill = plan.paints[0].fill_css.clone();
        for stroke in [None, Some("#badbad")] {
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            receipt.record_node_paints(&plan, 0, "A", 1, (Some(&fill), stroke));
            assert!(!plan.record_terminal(receipt));
        }
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        receipt.record_node_paints(&plan, 0, "A", 1, (Some(&fill), Some("#123456")));
        assert!(plan.record_terminal(receipt));
    }

    #[test]
    fn unsupported_text_palette_is_not_applicable_without_visible_label_terminals() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Text,
                    OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap(),
                ),
            ))
            .unwrap()
            .resolve(DiagramFamilyId::SANKEY);
        let config =
            crate::test_support::presentation_config(json!({"sankey":{"showValues":false}}));
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        for ids in [&[][..], &[""][..]] {
            let plan = SankeyNodePalettePlan::resolve(Some(&theme), &config, &layout(ids), &meter)
                .unwrap();
            let evidence = collect_evidence(&theme, |evidence| plan.record_evidence(evidence));
            assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
            assert!(evidence.applied().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    fn complete_typography_receipt(
        plan: &SankeyTypographyThemePlan,
        label_count: usize,
        outlined_labels: bool,
    ) -> SankeyTypographyThemeReceipt<'_> {
        let mut receipt = plan
            .begin_terminal_receipt(label_count, outlined_labels)
            .expect("typed Sankey typography receipt");
        receipt.record_css_emission(expected_typography_css_emission(plan));
        let surfaces = if outlined_labels {
            &[
                SankeyLabelSurface::OutlineBackground,
                SankeyLabelSurface::OutlineForeground,
            ][..]
        } else {
            &[SankeyLabelSurface::Plain][..]
        };
        for surface in surfaces {
            for _ in 0..label_count {
                receipt.record_label(*surface, true);
            }
        }
        receipt
    }

    fn typography_css_emission<'a>(
        font_family_css: &'a str,
        all_font_surfaces_match: bool,
        text_fill_css: &'a str,
        label_foreground_fill_css: &'a str,
    ) -> SankeyTypographyCssEmission<'a> {
        SankeyTypographyCssEmission::from_successful_writes(
            font_family_css,
            all_font_surfaces_match,
            text_fill_css,
            label_foreground_fill_css,
        )
    }

    fn expected_typography_css_emission(
        plan: &SankeyTypographyThemePlan,
    ) -> SankeyTypographyCssEmission<'_> {
        let text_fill_css = plan.text_fill_css().unwrap_or("#333");
        typography_css_emission(plan.font_family_css(), true, text_fill_css, text_fill_css)
    }

    fn palette_plan(config: serde_json::Value) -> (SankeyNodePalettePlan, ResolvedDiagramTheme) {
        let layout = layout(&["A", "B"]);
        let config = crate::test_support::presentation_config(config);
        let theme = resolved_node_palette(&["#123456", "#abcdef"]);
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let plan = SankeyNodePalettePlan::resolve(Some(&theme), &config, &layout, &work_meter)
            .expect("resolve Sankey test palette");
        (plan, theme)
    }

    #[test]
    fn explicit_node_color_owns_only_its_node() {
        let (plan, _) = palette_plan(json!({
            "sankey": { "nodeColors": { "A": "#fedcba" } }
        }));

        assert_eq!(plan.fill_for(0, "A"), Some("#fedcba"));
        assert_eq!(plan.fill_for(1, "B"), Some("#abcdef"));
    }

    #[test]
    fn terminal_receipt_requires_every_node_in_order_with_exact_fill() {
        let (plan, theme) = palette_plan(json!({}));
        let mut receipt = plan
            .begin_terminal_receipt()
            .expect("direct Sankey palette receipt");
        receipt.record_node(&plan, 0, "A", 1, Some("#123456"));
        receipt.record_node(&plan, 1, "B", 2, Some("#abcdef"));

        assert!(plan.record_terminal(receipt));
        let evidence = collect_evidence(&theme, |evidence| plan.record_evidence(evidence));
        assert_eq!(evidence.applied().len(), 1);
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn incomplete_or_mismatched_terminal_receipt_fails_closed() {
        for record_second in [false, true] {
            let (plan, theme) = palette_plan(json!({}));
            let mut receipt = plan
                .begin_terminal_receipt()
                .expect("direct Sankey palette receipt");
            receipt.record_node(&plan, 0, "A", 1, Some("#123456"));
            if record_second {
                receipt.record_node(&plan, 1, "B", 2, Some("#badbad"));
            }

            assert!(!plan.record_terminal(receipt));
            let evidence = collect_evidence(&theme, |evidence| plan.record_evidence(evidence));
            assert!(evidence.applied().is_empty());
            assert_eq!(evidence.residuals().len(), 1);
        }
    }

    #[test]
    fn typography_receipt_accepts_plain_and_outlined_label_passes() {
        for outlined_labels in [false, true] {
            let layout = layout(&["A", "B"]);
            let theme = resolved_typography(ThemeTextStyle::default().with_font_stack(
                FontStack::new(["SankeySans", "sans-serif"]).expect("valid Sankey test font stack"),
            ));
            let plan = SankeyTypographyThemePlan::resolve(
                Some(&theme),
                &crate::test_support::presentation_config(json!({})),
                2,
                &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
            )
            .unwrap();

            assert!(plan.record_terminal(complete_typography_receipt(
                &plan,
                layout.nodes.len(),
                outlined_labels,
            )));
            let evidence = collect_evidence(&theme, |evidence| plan.record_evidence(evidence));
            assert_eq!(evidence.applied().len(), 1);
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn typography_receipt_requires_matching_visible_ordinals_between_label_passes() {
        use crate::diagram_theme::{OrdinalSelector, ThemeRule, ThemeStylePatch};

        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        )
                        .with_ordinal(OrdinalSelector::Exact(1)),
                    ),
                ),
            )
            .unwrap()
            .resolve(DiagramFamilyId::SANKEY);
        for foreground in [[false, true], [true, false]] {
            let meter =
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
            let plan = SankeyTypographyThemePlan::resolve(
                Some(&theme),
                &crate::test_support::presentation_config(json!({})),
                2,
                &meter,
            )
            .unwrap();
            let mut receipt = plan.begin_terminal_receipt(2, true).unwrap();
            receipt.record_css_emission(expected_typography_css_emission(&plan));
            for visible in [false, true] {
                receipt.record_label(SankeyLabelSurface::OutlineBackground, visible);
            }
            for visible in foreground {
                receipt.record_label(SankeyLabelSurface::OutlineForeground, visible);
            }
            assert_eq!(plan.record_terminal(receipt), foreground == [false, true]);
        }
    }

    #[test]
    fn typography_receipt_rejects_missing_duplicate_and_reordered_milestones() {
        let theme = resolved_typography(ThemeTextStyle::default().with_font_stack(
            FontStack::single("SankeySans").expect("valid Sankey test font stack"),
        ));

        let missing_css = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &crate::test_support::presentation_config(json!({})),
            2,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();
        let mut receipt = missing_css
            .begin_terminal_receipt(2, false)
            .expect("typed Sankey typography receipt");
        for _ in 0..2 {
            receipt.record_label(SankeyLabelSurface::Plain, true);
        }
        assert!(!missing_css.record_terminal(receipt));
        assert_eq!(
            collect_evidence(&theme, |evidence| missing_css.record_evidence(evidence))
                .residuals()
                .len(),
            1
        );

        let duplicate_css = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &crate::test_support::presentation_config(json!({})),
            2,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();
        let mut receipt = duplicate_css
            .begin_terminal_receipt(1, false)
            .expect("typed Sankey typography receipt");
        receipt.record_css_emission(expected_typography_css_emission(&duplicate_css));
        receipt.record_css_emission(expected_typography_css_emission(&duplicate_css));
        receipt.record_label(SankeyLabelSurface::Plain, true);
        assert!(!duplicate_css.record_terminal(receipt));
        assert_eq!(
            collect_evidence(&theme, |evidence| duplicate_css.record_evidence(evidence))
                .residuals()
                .len(),
            1
        );

        let reordered = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &crate::test_support::presentation_config(json!({})),
            2,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();
        let mut receipt = reordered
            .begin_terminal_receipt(1, true)
            .expect("typed Sankey typography receipt");
        receipt.record_css_emission(expected_typography_css_emission(&reordered));
        receipt.record_label(SankeyLabelSurface::OutlineForeground, true);
        receipt.record_label(SankeyLabelSurface::OutlineBackground, true);
        assert!(!reordered.record_terminal(receipt));
        assert_eq!(
            collect_evidence(&theme, |evidence| reordered.record_evidence(evidence))
                .residuals()
                .len(),
            1
        );
    }

    #[test]
    fn typography_receipt_rejects_mismatched_css_values() {
        let theme = resolved_typography(ThemeTextStyle::default().with_font_stack(
            FontStack::single("SankeySans").expect("valid Sankey test font stack"),
        ));
        let wrong_font = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &crate::test_support::presentation_config(json!({})),
            2,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();
        let mut receipt = wrong_font
            .begin_terminal_receipt(1, false)
            .expect("typed Sankey typography receipt");
        receipt.record_css_emission(typography_css_emission(
            "WrongSankeyFamily",
            true,
            "#333",
            "#333",
        ));
        receipt.record_label(SankeyLabelSurface::Plain, true);
        assert!(!wrong_font.record_terminal(receipt));
        assert_eq!(
            collect_evidence(&theme, |evidence| wrong_font.record_evidence(evidence))
                .residuals()
                .len(),
            1
        );

        let inconsistent_surfaces = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &crate::test_support::presentation_config(json!({})),
            2,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();
        let mut receipt = inconsistent_surfaces
            .begin_terminal_receipt(1, false)
            .expect("typed Sankey typography receipt");
        receipt.record_css_emission(typography_css_emission(
            inconsistent_surfaces.font_family_css(),
            false,
            "#333",
            "#333",
        ));
        receipt.record_label(SankeyLabelSurface::Plain, true);
        assert!(!inconsistent_surfaces.record_terminal(receipt));
        assert_eq!(
            collect_evidence(&theme, |evidence| inconsistent_surfaces
                .record_evidence(evidence))
            .residuals()
            .len(),
            1
        );
    }

    #[test]
    fn typography_is_not_applicable_without_visible_labels() {
        let layout = layout(&[]);
        let theme = resolved_typography(ThemeTextStyle::default().with_font_stack(
            FontStack::single("SankeySans").expect("valid Sankey test font stack"),
        ));
        let plan = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &crate::test_support::presentation_config(json!({})),
            2,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();

        assert!(plan.record_terminal(complete_typography_receipt(
            &plan,
            layout.nodes.len(),
            false,
        )));
        let evidence = collect_evidence(&theme, |evidence| plan.record_evidence(evidence));
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
        assert!(evidence.residuals().is_empty());
    }
}
