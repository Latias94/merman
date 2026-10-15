use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use merman_core::MermaidConfig;

use super::{FlowchartConfigView, FlowchartLayoutSettings};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::text::TextStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowchartBaseTypographyOutcome {
    Inactive,
    Typed,
    ConfigOwned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FlowchartBaseTypographyOccurrence {
    Label(super::FlowchartSvgLabelOwner),
    DiagramTitle,
}

#[derive(Debug, Default)]
struct FlowchartBaseTypographyReceipt {
    stylesheet_font_stack_verified: bool,
    stylesheet_font_size_verified: bool,
    visible_text_occurrences: BTreeSet<FlowchartBaseTypographyOccurrence>,
    unidentified_visible_text: bool,
    font_stack_occurrences: BTreeSet<FlowchartBaseTypographyOccurrence>,
    font_stack_shadowed: BTreeSet<FlowchartBaseTypographyOccurrence>,
    font_stack_unverified: bool,
    font_size_occurrences: BTreeSet<FlowchartBaseTypographyOccurrence>,
    font_size_shadowed: BTreeSet<FlowchartBaseTypographyOccurrence>,
    font_size_unverified: bool,
}

#[derive(Debug)]
struct FlowchartBaseTypographyPlanInner {
    font_family_css: Box<str>,
    font_size_px: f64,
    html_font_size_px: f64,
    outcome: FlowchartBaseTypographyOutcome,
    unsupported_properties: BTreeSet<ThemeTypographyProperty>,
    typed_font_stack_requested: bool,
    typed_font_size_requested: bool,
    typed_font_stack_selected: bool,
    typed_font_size_selected: bool,
    terminal_receipt: Mutex<FlowchartBaseTypographyReceipt>,
}

/// One family-local winner shared by Flowchart/Swimlane layout, SVG emission, and evidence.
///
/// The module deliberately owns only family-wide `FontStack` and `FontSize`. Label-local source
/// styles and typed target rules retain their existing, more specific winner paths.
#[derive(Debug, Clone)]
pub(crate) struct FlowchartBaseTypographyPlan {
    inner: Arc<FlowchartBaseTypographyPlanInner>,
}

#[derive(Debug)]
pub(crate) struct FlowchartBaseTypographyStyles {
    pub(crate) font_family: String,
    pub(crate) font_size: f64,
    pub(crate) text_style: TextStyle,
    pub(crate) html_label_text_style: TextStyle,
}

/// One concrete visible-label emission observed by the Flowchart SVG writer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartBaseTypographyLabelEmission {
    owner: Option<FlowchartBaseTypographyOccurrence>,
    typography_applicable: bool,
    writer_verified: bool,
    source_font_stack: super::FlowchartSourceFacetStatus,
    source_font_size: super::FlowchartSourceFacetStatus,
    target_font_stack_selected: bool,
    target_font_size_selected: bool,
    local_font_stack_shadowed: bool,
    local_font_size_shadowed: bool,
}

impl FlowchartBaseTypographyLabelEmission {
    pub(crate) const fn new(
        owner: Option<super::FlowchartSvgLabelOwner>,
        typography_applicable: bool,
        writer_verified: bool,
    ) -> Self {
        Self {
            owner: match owner {
                Some(owner) => Some(FlowchartBaseTypographyOccurrence::Label(owner)),
                None => None,
            },
            typography_applicable,
            writer_verified,
            source_font_stack: super::FlowchartSourceFacetStatus::Absent,
            source_font_size: super::FlowchartSourceFacetStatus::Absent,
            target_font_stack_selected: false,
            target_font_size_selected: false,
            local_font_stack_shadowed: false,
            local_font_size_shadowed: false,
        }
    }

    pub(crate) const fn diagram_title() -> Self {
        Self {
            owner: Some(FlowchartBaseTypographyOccurrence::DiagramTitle),
            typography_applicable: true,
            writer_verified: true,
            source_font_stack: super::FlowchartSourceFacetStatus::Absent,
            source_font_size: super::FlowchartSourceFacetStatus::Absent,
            target_font_stack_selected: false,
            target_font_size_selected: false,
            local_font_stack_shadowed: false,
            local_font_size_shadowed: true,
        }
    }

    pub(crate) const fn with_source_facets(
        mut self,
        font_stack: super::FlowchartSourceFacetStatus,
        font_size: super::FlowchartSourceFacetStatus,
    ) -> Self {
        self.source_font_stack = font_stack;
        self.source_font_size = font_size;
        self
    }

    pub(crate) const fn with_target_selection(mut self, font_stack: bool, font_size: bool) -> Self {
        self.target_font_stack_selected = font_stack;
        self.target_font_size_selected = font_size;
        self
    }
}

impl FlowchartBaseTypographyPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let config = FlowchartConfigView::new(effective_config.as_value());
        let configured_font_family = config.font_family();
        let configured_font_size = config.render_font_size();
        let configured_text_style =
            config.render_text_style(&configured_font_family, configured_font_size);
        let configured_html_style =
            config.html_label_measurement_base_style(&configured_text_style);

        let Some(theme) = theme else {
            return Self::new(
                configured_font_family,
                configured_font_size,
                configured_html_style.font_size,
                FlowchartBaseTypographyOutcome::Inactive,
                BTreeSet::new(),
                false,
                false,
                false,
                false,
            );
        };

        let ownership = super::flowchart_typography_config_ownership(effective_config);
        let mut typed_font_stack = false;
        let mut typed_font_size = false;
        let mut unsupported_properties = BTreeSet::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_size = true;
                }
                FamilyThemeMechanism::BaseTypography(property)
                    if route.disposition() == FamilyThemeDisposition::Unsupported =>
                {
                    unsupported_properties.insert(property);
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let typed_font_stack_applied = typed_font_stack && !ownership.font_stack;
        let typed_font_size_applied = typed_font_size && !ownership.font_size;
        let font_family = if typed_font_stack_applied {
            theme.typography().font_stack().as_css()
        } else {
            configured_font_family
        };
        let font_size = if typed_font_size_applied {
            f64::from(theme.typography().font_size_px())
        } else {
            configured_font_size
        };
        let html_font_size = if typed_font_size_applied {
            font_size
        } else {
            configured_html_style.font_size
        };
        let requested = typed_font_stack || typed_font_size;
        let applied = typed_font_stack_applied || typed_font_size_applied;
        let outcome = if applied {
            FlowchartBaseTypographyOutcome::Typed
        } else if requested {
            FlowchartBaseTypographyOutcome::ConfigOwned
        } else {
            FlowchartBaseTypographyOutcome::Inactive
        };

        Self::new(
            font_family,
            font_size,
            html_font_size,
            outcome,
            unsupported_properties,
            typed_font_stack,
            typed_font_size,
            typed_font_stack_applied,
            typed_font_size_applied,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The plan stores independent typography ownership and measurement inputs"
    )]
    fn new(
        font_family_css: String,
        font_size_px: f64,
        html_font_size_px: f64,
        outcome: FlowchartBaseTypographyOutcome,
        unsupported_properties: BTreeSet<ThemeTypographyProperty>,
        typed_font_stack_requested: bool,
        typed_font_size_requested: bool,
        typed_font_stack_selected: bool,
        typed_font_size_selected: bool,
    ) -> Self {
        Self {
            inner: Arc::new(FlowchartBaseTypographyPlanInner {
                font_family_css: font_family_css.into_boxed_str(),
                font_size_px,
                html_font_size_px,
                outcome,
                unsupported_properties,
                typed_font_stack_requested,
                typed_font_size_requested,
                typed_font_stack_selected,
                typed_font_size_selected,
                terminal_receipt: Mutex::new(FlowchartBaseTypographyReceipt::default()),
            }),
        }
    }

    pub(crate) fn layout_settings(
        &self,
        effective_config: &serde_json::Value,
    ) -> FlowchartLayoutSettings {
        let config = FlowchartConfigView::new(effective_config);
        let mut settings = config.layout_settings();
        self.apply_to_styles(
            &mut settings.text_style,
            &mut settings.html_label_text_style,
        );
        settings
    }

    pub(crate) fn render_styles(
        &self,
        effective_config: &serde_json::Value,
    ) -> FlowchartBaseTypographyStyles {
        let config = FlowchartConfigView::new(effective_config);
        let mut text_style =
            config.render_text_style(self.inner.font_family_css.as_ref(), self.inner.font_size_px);
        let mut html_label_text_style = config.html_label_measurement_base_style(&text_style);
        self.apply_to_styles(&mut text_style, &mut html_label_text_style);
        FlowchartBaseTypographyStyles {
            font_family: self.inner.font_family_css.to_string(),
            font_size: self.inner.font_size_px,
            text_style,
            html_label_text_style,
        }
    }

    fn apply_to_styles(&self, text_style: &mut TextStyle, html_label_text_style: &mut TextStyle) {
        text_style.font_family = Some(self.inner.font_family_css.to_string());
        text_style.font_size = self.inner.font_size_px;
        html_label_text_style.font_family = Some(self.inner.font_family_css.to_string());
        html_label_text_style.font_size = self.inner.html_font_size_px;
    }

    pub(crate) fn requires_terminal_evidence(&self) -> bool {
        !matches!(self.inner.outcome, FlowchartBaseTypographyOutcome::Inactive)
            || !self.inner.unsupported_properties.is_empty()
    }

    pub(crate) fn begin_terminal_emission(&self) {
        if !self.requires_terminal_evidence() {
            return;
        }
        *self
            .inner
            .terminal_receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            FlowchartBaseTypographyReceipt::default();
    }

    pub(crate) fn record_stylesheet_emission(
        &self,
        emitted_font_family: &str,
        emitted_font_size_px: f64,
    ) {
        if !self.requires_terminal_evidence() {
            return;
        }
        let mut receipt = self
            .inner
            .terminal_receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        receipt.stylesheet_font_stack_verified |=
            emitted_font_family == self.inner.font_family_css.as_ref();
        receipt.stylesheet_font_size_verified |=
            emitted_font_size_px.to_bits() == self.inner.font_size_px.to_bits();
    }

    pub(crate) fn record_label_emission(&self, emission: FlowchartBaseTypographyLabelEmission) {
        if !self.requires_terminal_evidence() || !emission.typography_applicable {
            return;
        }
        let mut receipt = self
            .inner
            .terminal_receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(owner) = emission.owner {
            receipt.visible_text_occurrences.insert(owner);
        } else {
            receipt.unidentified_visible_text = true;
        }
        if self.inner.typed_font_stack_selected {
            match property_occurrence_status(
                emission.owner,
                emission.writer_verified,
                emission.source_font_stack,
                emission.target_font_stack_selected || emission.local_font_stack_shadowed,
            ) {
                FlowchartBaseTypographyOccurrenceStatus::Applied(owner) => {
                    receipt.font_stack_occurrences.insert(owner);
                }
                FlowchartBaseTypographyOccurrenceStatus::Shadowed(owner) => {
                    receipt.font_stack_shadowed.insert(owner);
                }
                FlowchartBaseTypographyOccurrenceStatus::Unverified => {
                    receipt.font_stack_unverified = true;
                }
            }
        }
        if self.inner.typed_font_size_selected {
            match property_occurrence_status(
                emission.owner,
                emission.writer_verified,
                emission.source_font_size,
                emission.target_font_size_selected || emission.local_font_size_shadowed,
            ) {
                FlowchartBaseTypographyOccurrenceStatus::Applied(owner) => {
                    receipt.font_size_occurrences.insert(owner);
                }
                FlowchartBaseTypographyOccurrenceStatus::Shadowed(owner) => {
                    receipt.font_size_shadowed.insert(owner);
                }
                FlowchartBaseTypographyOccurrenceStatus::Unverified => {
                    receipt.font_size_unverified = true;
                }
            }
        }
    }

    pub(crate) fn finish_evidence(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> FamilyThemeEvidence {
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let receipt = self
            .inner
            .terminal_receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if receipt.visible_text_occurrences.is_empty() && !receipt.unidentified_visible_text {
            if self.inner.typed_font_stack_requested {
                evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(
                    ThemeTypographyProperty::FontStack,
                ));
            }
            if self.inner.typed_font_size_requested {
                evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(
                    ThemeTypographyProperty::FontSize,
                ));
            }
            for property in &self.inner.unsupported_properties {
                evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(*property));
            }
            return evidence;
        }

        match self.inner.outcome {
            FlowchartBaseTypographyOutcome::Typed => {
                let font_stack = property_terminal_status(
                    self.inner.typed_font_stack_selected,
                    receipt.stylesheet_font_stack_verified,
                    &receipt.visible_text_occurrences,
                    receipt.unidentified_visible_text,
                    &receipt.font_stack_occurrences,
                    &receipt.font_stack_shadowed,
                    receipt.font_stack_unverified,
                );
                let font_size = property_terminal_status(
                    self.inner.typed_font_size_selected,
                    receipt.stylesheet_font_size_verified,
                    &receipt.visible_text_occurrences,
                    receipt.unidentified_visible_text,
                    &receipt.font_size_occurrences,
                    &receipt.font_size_shadowed,
                    receipt.font_size_unverified,
                );
                for (property, requested, selected, status) in [
                    (
                        ThemeTypographyProperty::FontStack,
                        self.inner.typed_font_stack_requested,
                        self.inner.typed_font_stack_selected,
                        font_stack,
                    ),
                    (
                        ThemeTypographyProperty::FontSize,
                        self.inner.typed_font_size_requested,
                        self.inner.typed_font_size_selected,
                        font_size,
                    ),
                ] {
                    if !requested {
                        continue;
                    }
                    let key = FamilyThemeMechanismKey::Typography(property);
                    if !selected || status.is_not_applicable() {
                        evidence.mark_not_applicable(key);
                    } else if status.is_applied() {
                        evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                    } else {
                        evidence
                            .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                    }
                }
            }
            FlowchartBaseTypographyOutcome::ConfigOwned => {
                for (property, requested) in [
                    (
                        ThemeTypographyProperty::FontStack,
                        self.inner.typed_font_stack_requested,
                    ),
                    (
                        ThemeTypographyProperty::FontSize,
                        self.inner.typed_font_size_requested,
                    ),
                ] {
                    if requested {
                        evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(property));
                    }
                }
            }
            FlowchartBaseTypographyOutcome::Inactive => {}
        }
        for property in &self.inner.unsupported_properties {
            evidence.mark_residual(
                FamilyThemeMechanismKey::Typography(*property),
                FamilyThemeResidualReason::UnsupportedTypography,
            );
        }
        evidence
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowchartBaseTypographyOccurrenceStatus {
    Applied(FlowchartBaseTypographyOccurrence),
    Shadowed(FlowchartBaseTypographyOccurrence),
    Unverified,
}

fn property_occurrence_status(
    owner: Option<FlowchartBaseTypographyOccurrence>,
    writer_verified: bool,
    source: super::FlowchartSourceFacetStatus,
    target_selected: bool,
) -> FlowchartBaseTypographyOccurrenceStatus {
    let Some(owner) = owner else {
        return FlowchartBaseTypographyOccurrenceStatus::Unverified;
    };
    if !source.is_absent() || target_selected {
        FlowchartBaseTypographyOccurrenceStatus::Shadowed(owner)
    } else if writer_verified {
        FlowchartBaseTypographyOccurrenceStatus::Applied(owner)
    } else {
        FlowchartBaseTypographyOccurrenceStatus::Unverified
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowchartBaseTypographyPropertyStatus {
    Inactive,
    Applied,
    NotApplicable,
    Unverified,
}

impl FlowchartBaseTypographyPropertyStatus {
    const fn is_applied(self) -> bool {
        matches!(self, Self::Applied)
    }

    const fn is_not_applicable(self) -> bool {
        matches!(self, Self::Inactive | Self::NotApplicable)
    }
}

fn property_terminal_status(
    selected: bool,
    stylesheet_verified: bool,
    visible: &BTreeSet<FlowchartBaseTypographyOccurrence>,
    unidentified_visible: bool,
    applied: &BTreeSet<FlowchartBaseTypographyOccurrence>,
    shadowed: &BTreeSet<FlowchartBaseTypographyOccurrence>,
    unverified: bool,
) -> FlowchartBaseTypographyPropertyStatus {
    if !selected {
        return FlowchartBaseTypographyPropertyStatus::Inactive;
    }
    if !stylesheet_verified || unidentified_visible || unverified || visible.is_empty() {
        return FlowchartBaseTypographyPropertyStatus::Unverified;
    }
    if !visible
        .iter()
        .all(|owner| applied.contains(owner) || shadowed.contains(owner))
    {
        return FlowchartBaseTypographyPropertyStatus::Unverified;
    }
    if !applied.is_empty() {
        return FlowchartBaseTypographyPropertyStatus::Applied;
    }
    if visible.iter().all(|owner| shadowed.contains(owner)) {
        return FlowchartBaseTypographyPropertyStatus::NotApplicable;
    }
    FlowchartBaseTypographyPropertyStatus::Unverified
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, FontStyle, ThemeTextStyle,
        TypographySpec,
    };

    fn typed_plan(config: MermaidConfig) -> FlowchartBaseTypographyPlan {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("valid test font"))
            .with_font_size_px(23.0)
            .expect("valid test font size");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::FLOWCHART, typography),
            ))
            .expect("compile Flowchart base typography test theme");
        FlowchartBaseTypographyPlan::resolve(
            Some(&theme.resolve(DiagramFamilyId::FLOWCHART)),
            &config,
        )
    }

    #[test]
    fn typed_base_typography_overrides_layout_and_html_measurement_styles() {
        let plan = typed_plan(MermaidConfig::from_value(serde_json::json!({})));
        let settings = plan.layout_settings(&serde_json::json!({}));

        assert_eq!(
            settings.text_style.font_family.as_deref(),
            Some("Excalifont")
        );
        assert_eq!(settings.text_style.font_size, 23.0);
        assert_eq!(
            settings.html_label_text_style.font_family.as_deref(),
            Some("Excalifont")
        );
        assert_eq!(settings.html_label_text_style.font_size, 23.0);
    }

    #[test]
    fn inactive_base_typography_does_not_request_terminal_evidence() {
        let plan = FlowchartBaseTypographyPlan::resolve(
            None,
            &MermaidConfig::from_value(serde_json::json!({})),
        );

        assert!(!plan.requires_terminal_evidence());
        plan.record_stylesheet_emission("ignored", 99.0);
        plan.record_label_emission(FlowchartBaseTypographyLabelEmission::new(
            Some(super::super::FlowchartSvgLabelOwner::Node(0)),
            true,
            true,
        ));

        let receipt = plan
            .inner
            .terminal_receipt
            .lock()
            .expect("inactive receipt lock");
        assert!(!receipt.stylesheet_font_stack_verified);
        assert!(!receipt.stylesheet_font_size_verified);
        assert!(receipt.visible_text_occurrences.is_empty());
    }

    #[test]
    fn property_evidence_requires_every_visible_occurrence_to_be_accounted() {
        let first =
            FlowchartBaseTypographyOccurrence::Label(super::super::FlowchartSvgLabelOwner::Node(0));
        let second =
            FlowchartBaseTypographyOccurrence::Label(super::super::FlowchartSvgLabelOwner::Node(1));
        let visible = BTreeSet::from([first, second]);

        assert_eq!(
            property_terminal_status(
                true,
                true,
                &visible,
                false,
                &BTreeSet::from([first]),
                &BTreeSet::new(),
                false,
            ),
            FlowchartBaseTypographyPropertyStatus::Unverified,
        );
        assert_eq!(
            property_terminal_status(
                true,
                true,
                &visible,
                false,
                &BTreeSet::from([first]),
                &BTreeSet::from([second]),
                false,
            ),
            FlowchartBaseTypographyPropertyStatus::Applied,
        );
    }

    #[test]
    fn diagram_title_is_a_real_base_font_stack_occurrence() {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("valid test font"));
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::FLOWCHART, typography),
            ))
            .expect("compile Flowchart title typography test theme");
        let resolved = theme.resolve(DiagramFamilyId::FLOWCHART);
        let plan = FlowchartBaseTypographyPlan::resolve(
            Some(&resolved),
            &MermaidConfig::from_value(serde_json::json!({})),
        );

        plan.begin_terminal_emission();
        plan.record_stylesheet_emission(
            plan.inner.font_family_css.as_ref(),
            plan.inner.font_size_px,
        );
        plan.record_label_emission(FlowchartBaseTypographyLabelEmission::diagram_title());

        let evidence = plan.finish_evidence(Some(&resolved));
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Typography(
                ThemeTypographyProperty::FontStack,
            )],
        );
    }

    #[test]
    fn unsupported_base_typography_properties_do_not_block_typed_properties() {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("valid test font"))
            .with_font_size_px(23.0)
            .expect("valid test font size")
            .with_font_weight(700)
            .expect("valid test font weight")
            .with_font_style(FontStyle::Italic);
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::FLOWCHART, typography),
            ))
            .expect("compile mixed Flowchart typography test theme");
        let resolved = theme.resolve(DiagramFamilyId::FLOWCHART);
        let plan = FlowchartBaseTypographyPlan::resolve(
            Some(&resolved),
            &MermaidConfig::from_value(serde_json::json!({})),
        );

        plan.begin_terminal_emission();
        plan.record_stylesheet_emission(
            plan.inner.font_family_css.as_ref(),
            plan.inner.font_size_px,
        );
        plan.record_label_emission(FlowchartBaseTypographyLabelEmission::new(
            Some(super::super::FlowchartSvgLabelOwner::Node(0)),
            true,
            true,
        ));

        let evidence = plan.finish_evidence(Some(&resolved));
        assert!(
            evidence
                .applied()
                .contains(&FamilyThemeMechanismKey::Typography(
                    ThemeTypographyProperty::FontStack,
                ))
        );
        assert!(
            evidence
                .applied()
                .contains(&FamilyThemeMechanismKey::Typography(
                    ThemeTypographyProperty::FontSize,
                ))
        );
        for property in [
            ThemeTypographyProperty::FontWeight,
            ThemeTypographyProperty::FontStyle,
        ] {
            let key = FamilyThemeMechanismKey::Typography(property);
            assert!(
                evidence
                    .residuals()
                    .iter()
                    .any(|residual| residual.key() == &key)
            );
        }
    }
}
