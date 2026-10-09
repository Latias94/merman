use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ResolvedStyleProperty, ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, resolve_direct_static_fill,
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
    evidence: FamilyThemeEvidence,
    text_fill: Option<DirectStaticPaint>,
    text_rules: BTreeMap<usize, SankeyTextRuleObservation>,
    config_owns_text_fill: bool,
    terminal_receipt: OnceLock<SankeyTypographyTerminalSeal>,
}

impl SankeyTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        label_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let config_owns_text_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.textColor",
        );
        let mut text_fill = None;
        let mut text_rules = BTreeMap::<usize, SankeyTextRuleObservation>::new();
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
            let mut has_ordinals = false;
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
                    let observation = text_rules
                        .get_mut(&origin.rule_index())
                        .expect("registered text rule");
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
            evidence: FamilyThemeEvidence::from_theme(theme),
            text_fill,
            text_rules,
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

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            if self.inherited_font_stack.typed_font_stack_requested() {
                self.inherited_font_stack
                    .mark_unsupported_typography_evidence(&mut evidence, true);
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
            for (index, observation) in &self.text_rules {
                let key = FamilyThemeMechanismKey::Rule {
                    index: *index,
                    target: ThemeTarget::Text,
                };
                if let Some(reason) = observation.residual {
                    evidence.mark_residual(key, reason);
                } else if observation.fill_pending {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
            return evidence;
        };
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, receipt.has_visible_label);
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
        for (index, observation) in &self.text_rules {
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
        evidence
    }
}

#[derive(Debug, Default)]
struct SankeyTextRuleObservation {
    fill_pending: bool,
    residual: Option<FamilyThemeResidualReason>,
}

/// Milestone issued only after the final Sankey stylesheet writer completes successfully.
#[derive(Debug)]
pub(crate) struct SankeyTypographyCssEmission {
    font_family_css: String,
    all_font_surfaces_match: bool,
    text_fill_css: String,
    label_foreground_fill_css: String,
}

impl SankeyTypographyCssEmission {
    pub(crate) fn from_successful_writes(
        font_family_css: String,
        all_font_surfaces_match: bool,
        text_fill_css: String,
        label_foreground_fill_css: String,
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
    valid: bool,
}

#[derive(Debug)]
struct SankeyTypographyTerminalSeal {
    has_visible_label: bool,
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
            valid: true,
        }
    }

    pub(crate) fn record_css_emission(&mut self, emission: SankeyTypographyCssEmission) {
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
            && self.labels_in_pass == 0)
            .then_some(SankeyTypographyTerminalSeal {
                has_visible_label: self.has_visible_label,
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
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    terminal_capabilities: OnceLock<BTreeSet<ThemeCapability>>,
}

impl SankeyNodePalettePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &SankeyDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(effective_config.as_value(), layout);
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
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
            FamilyThemeDisposition::LegacyCompatibility => {}
        }

        Ok(plan)
    }

    fn baseline(effective_config: &serde_json::Value, layout: &SankeyDiagramLayout) -> Self {
        let render_settings = SankeyConfigView::new(effective_config).render_settings();
        let node_colors = render_settings.node_colors;
        let mut node_indices = HashMap::with_capacity(layout.nodes.len());
        let mut paints = Vec::with_capacity(layout.nodes.len());

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
            paints.push(SankeyNodePaint {
                id: node.id.clone(),
                ordinal: node_index + 1,
                fill_css: explicit_fill
                    .unwrap_or(TABLEAU10[node_index % TABLEAU10.len()])
                    .to_string(),
                source_owned: explicit_fill.is_some(),
                typed_capability: None,
            });
        }

        Self {
            node_indices,
            links: None,
            literal_link_paint: "".into(),
            #[cfg(test)]
            endpoint_lookups: 0,
            paints,
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            terminal_capabilities: OnceLock::new(),
        }
    }

    pub(crate) fn fill_for(&self, node_index: usize, node_id: &str) -> Option<&str> {
        self.paints
            .get(node_index)
            .filter(|paint| paint.id == node_id)
            .map(|paint| paint.fill_css.as_str())
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
            work_meter.checkpoint(merman_core::OperationPhase::Layout)?;
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
        self.palette_key
            .as_ref()
            .map(|_| SankeyNodePaletteReceipt::new(self.paints.len()))
    }

    pub(crate) fn record_terminal(&self, receipt: SankeyNodePaletteReceipt) -> bool {
        if self.palette_key.is_none() || !receipt.proves_complete(&self.paints) {
            return false;
        }
        self.terminal_capabilities
            .set(receipt.applied_capabilities)
            .is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(key) = self.palette_key.clone() else {
            return evidence;
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
        evidence
    }
}

/// Writer-owned proof that every semantic Sankey node emitted its planned terminal rectangle fill.
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

    pub(crate) fn record_node(
        &mut self,
        plan: &SankeyNodePalettePlan,
        emitted_index: usize,
        emitted_id: &str,
        emitted_ordinal: usize,
        emitted_fill: Option<&str>,
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
            && emitted_fill.is_some_and(|fill| fill == paint.fill_css);
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
        let config = MermaidConfig::from_value(json!({
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

        let ignored = MermaidConfig::from_value(json!({"sankey": {
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
                .map(|color| ThemeColorValue::parse(*color).expect("valid Sankey test color")),
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

    fn typography_css_emission(
        font_family_css: &str,
        all_font_surfaces_match: bool,
        text_fill_css: &str,
        label_foreground_fill_css: &str,
    ) -> SankeyTypographyCssEmission {
        SankeyTypographyCssEmission::from_successful_writes(
            font_family_css.to_owned(),
            all_font_surfaces_match,
            text_fill_css.to_owned(),
            label_foreground_fill_css.to_owned(),
        )
    }

    fn expected_typography_css_emission(
        plan: &SankeyTypographyThemePlan,
    ) -> SankeyTypographyCssEmission {
        let text_fill_css = plan.text_fill_css().unwrap_or("#333");
        typography_css_emission(plan.font_family_css(), true, text_fill_css, text_fill_css)
    }

    fn palette_plan(config: serde_json::Value) -> SankeyNodePalettePlan {
        let layout = layout(&["A", "B"]);
        let config = MermaidConfig::from_value(config);
        let theme = resolved_node_palette(&["#123456", "#abcdef"]);
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        SankeyNodePalettePlan::resolve(Some(&theme), &config, &layout, &work_meter)
            .expect("resolve Sankey test palette")
    }

    #[test]
    fn explicit_node_color_owns_only_its_node() {
        let plan = palette_plan(json!({
            "sankey": { "nodeColors": { "A": "#fedcba" } }
        }));

        assert_eq!(plan.fill_for(0, "A"), Some("#fedcba"));
        assert_eq!(plan.fill_for(1, "B"), Some("#abcdef"));
    }

    #[test]
    fn terminal_receipt_requires_every_node_in_order_with_exact_fill() {
        let plan = palette_plan(json!({}));
        let mut receipt = plan
            .begin_terminal_receipt()
            .expect("direct Sankey palette receipt");
        receipt.record_node(&plan, 0, "A", 1, Some("#123456"));
        receipt.record_node(&plan, 1, "B", 2, Some("#abcdef"));

        assert!(plan.record_terminal(receipt));
        let evidence = plan.finish_evidence();
        assert_eq!(evidence.applied().len(), 1);
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn incomplete_or_mismatched_terminal_receipt_fails_closed() {
        for record_second in [false, true] {
            let plan = palette_plan(json!({}));
            let mut receipt = plan
                .begin_terminal_receipt()
                .expect("direct Sankey palette receipt");
            receipt.record_node(&plan, 0, "A", 1, Some("#123456"));
            if record_second {
                receipt.record_node(&plan, 1, "B", 2, Some("#badbad"));
            }

            assert!(!plan.record_terminal(receipt));
            let evidence = plan.finish_evidence();
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
                &MermaidConfig::from_value(json!({})),
                2,
                &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
            )
            .unwrap();

            assert!(plan.record_terminal(complete_typography_receipt(
                &plan,
                layout.nodes.len(),
                outlined_labels,
            )));
            let evidence = plan.finish_evidence();
            assert_eq!(evidence.applied().len(), 1);
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn typography_receipt_rejects_missing_duplicate_and_reordered_milestones() {
        let theme = resolved_typography(ThemeTextStyle::default().with_font_stack(
            FontStack::single("SankeySans").expect("valid Sankey test font stack"),
        ));

        let missing_css = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::from_value(json!({})),
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
        assert_eq!(missing_css.finish_evidence().residuals().len(), 1);

        let duplicate_css = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::from_value(json!({})),
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
        assert_eq!(duplicate_css.finish_evidence().residuals().len(), 1);

        let reordered = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::from_value(json!({})),
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
        assert_eq!(reordered.finish_evidence().residuals().len(), 1);
    }

    #[test]
    fn typography_receipt_rejects_mismatched_css_values() {
        let theme = resolved_typography(ThemeTextStyle::default().with_font_stack(
            FontStack::single("SankeySans").expect("valid Sankey test font stack"),
        ));
        let wrong_font = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::from_value(json!({})),
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
        assert_eq!(wrong_font.finish_evidence().residuals().len(), 1);

        let inconsistent_surfaces = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::from_value(json!({})),
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
        assert_eq!(inconsistent_surfaces.finish_evidence().residuals().len(), 1);
    }

    #[test]
    fn typography_is_not_applicable_without_visible_labels() {
        let layout = layout(&[]);
        let theme = resolved_typography(ThemeTextStyle::default().with_font_stack(
            FontStack::single("SankeySans").expect("valid Sankey test font stack"),
        ));
        let plan = SankeyTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::from_value(json!({})),
            2,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap();

        assert!(plan.record_terminal(complete_typography_receipt(
            &plan,
            layout.nodes.len(),
            false,
        )));
        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
        assert!(evidence.residuals().is_empty());
    }
}
