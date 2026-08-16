use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};

#[derive(Debug, Clone, Default)]
struct SequenceThemeEvidenceState {
    actor_count: usize,
    actor_fill_emitted: bool,
    actor_fill_unhandled: bool,
    actor_fill_overridden: bool,
    actor_stroke_emitted: bool,
    actor_stroke_unhandled: bool,
    actor_stroke_overridden: bool,
    actor_style_receipt: SequenceActorThemeReceipt,
    lifeline: SequenceLifelineThemeState,
    message: SequenceMessageThemeState,
    note: SequenceStaticRectThemeState,
    activation: SequenceStaticRectThemeState,
    typography: Option<SequenceTypographyThemeReceipt>,
}

/// Winner facts produced by the terminal Sequence Actor writer.
///
/// Static and ordinal winners remain separate because a static rule is resolved against the
/// family default while ordinal selectors are evaluated against each concrete one-based actor
/// ordinal. The writer resolves each ordinal exactly once and records every facet winner from
/// that result; evidence finalization only consumes these facts and performs no selector scan.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceActorThemeReceipt {
    static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    ordinal_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
}

impl SequenceActorThemeReceipt {
    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
    }

    pub(crate) fn record_ordinal_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.ordinal_winners, style);
    }

    fn merge(&mut self, other: Self) {
        self.static_winners.extend(other.static_winners);
        self.ordinal_winners.extend(other.ordinal_winners);
    }

    fn route_won(
        &self,
        rule_index: usize,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        let property = style_property_for_facet(facet);
        match selector {
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default),
            } => self.static_winners.contains(&(rule_index, property)),
            FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            } => self.ordinal_winners.contains(&(rule_index, property)),
            FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => {
                false
            }
        }
    }
}

/// Winner and terminal-emission facts produced by the Sequence Lifeline writer.
///
/// Mermaid maps both Lifeline fill and stroke to `actorLineColor`, with an explicit stroke winner
/// taking precedence over fill. Candidate and emitted counts remain separate so missing actor or
/// layout paths cannot manufacture positive evidence from the generated CSS alone.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceLifelineThemeReceipt {
    static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    line_candidates: usize,
    emitted_lines: usize,
}

impl SequenceLifelineThemeReceipt {
    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
    }

    pub(crate) fn record_line_candidate(&mut self) {
        self.line_candidates = self.line_candidates.saturating_add(1);
    }

    pub(crate) fn record_line_emission(&mut self) {
        self.emitted_lines = self.emitted_lines.saturating_add(1);
    }

    fn merge(&mut self, other: Self) {
        self.static_winners.extend(other.static_winners);
        self.line_candidates = self.line_candidates.max(other.line_candidates);
        self.emitted_lines = self.emitted_lines.max(other.emitted_lines);
    }

    fn route_won(
        &self,
        rule_index: usize,
        _selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        self.static_winners
            .contains(&(rule_index, style_property_for_facet(facet)))
    }
}

#[derive(Debug, Clone, Default)]
struct SequenceLifelineThemeState {
    paint_emitted: bool,
    stroke_width_emitted: bool,
    paint_overridden: bool,
    selected_property: Option<ResolvedStyleProperty>,
    receipt: SequenceLifelineThemeReceipt,
}

impl SequenceLifelineThemeState {
    fn merge(&mut self, emission: SequenceLifelineThemeEmission) {
        self.paint_emitted |= emission.paint_emitted;
        self.stroke_width_emitted |= emission.stroke_width_emitted;
        self.paint_overridden |= emission.paint_overridden;
        if let Some(selected_property) = emission.selected_property {
            debug_assert!(
                self.selected_property.is_none()
                    || self.selected_property == Some(selected_property),
                "one Sequence artifact must resolve one stable Lifeline paint winner"
            );
            self.selected_property.get_or_insert(selected_property);
        }
        self.receipt.merge(emission.receipt);
    }
}

/// Complete writer-owned emission facts for the Sequence Lifeline style tranche.
#[derive(Debug)]
pub(crate) struct SequenceLifelineThemeEmission {
    paint_emitted: bool,
    stroke_width_emitted: bool,
    paint_overridden: bool,
    selected_property: Option<ResolvedStyleProperty>,
    receipt: SequenceLifelineThemeReceipt,
}

impl SequenceLifelineThemeEmission {
    pub(crate) fn from_terminal_writer(
        typed_stroke: Option<&str>,
        typed_stroke_width: Option<f32>,
        selected_property: Option<ResolvedStyleProperty>,
        paint_overridden: bool,
        receipt: SequenceLifelineThemeReceipt,
    ) -> Self {
        let has_lines = receipt.line_candidates != 0;
        let complete_line_emission = has_lines && receipt.emitted_lines == receipt.line_candidates;
        Self {
            paint_emitted: complete_line_emission && typed_stroke.is_some(),
            stroke_width_emitted: complete_line_emission && typed_stroke_width.is_some(),
            paint_overridden: has_lines && paint_overridden,
            selected_property,
            receipt,
        }
    }
}

/// Winner and terminal-emission facts produced by the Sequence Message line writer.
///
/// Message directly owns only unqualified static stroke in this tranche. The receipt also retains
/// fill winners because Mermaid's legacy `signalColor` projection selects stroke-or-fill, and a
/// direct stroke winner must explicitly account for any shadowed fill rule. Candidate and emitted
/// counts stay separate so a compiled route cannot claim evidence unless every concrete terminal
/// line or path reached the writer-owned checkpoint.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceMessageThemeReceipt {
    static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    line_candidates: usize,
    emitted_lines: usize,
}

impl SequenceMessageThemeReceipt {
    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
    }

    pub(crate) fn record_line_candidate(&mut self) {
        self.line_candidates = self.line_candidates.saturating_add(1);
    }

    pub(crate) fn record_line_emission(&mut self) {
        self.emitted_lines = self.emitted_lines.saturating_add(1);
    }

    fn merge(&mut self, other: Self) {
        self.static_winners.extend(other.static_winners);
        self.line_candidates = self.line_candidates.max(other.line_candidates);
        self.emitted_lines = self.emitted_lines.max(other.emitted_lines);
    }

    fn route_won(
        &self,
        rule_index: usize,
        _selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        self.static_winners
            .contains(&(rule_index, style_property_for_facet(facet)))
    }
}

#[derive(Debug, Clone, Default)]
struct SequenceMessageThemeState {
    stroke_emitted: bool,
    stroke_overridden: bool,
    receipt: SequenceMessageThemeReceipt,
}

impl SequenceMessageThemeState {
    fn merge(&mut self, emission: SequenceMessageThemeEmission) {
        self.stroke_emitted |= emission.stroke_emitted;
        self.stroke_overridden |= emission.stroke_overridden;
        self.receipt.merge(emission.receipt);
    }
}

/// Complete writer-owned emission facts for the Sequence Message stroke tranche.
#[derive(Debug)]
pub(crate) struct SequenceMessageThemeEmission {
    stroke_emitted: bool,
    stroke_overridden: bool,
    receipt: SequenceMessageThemeReceipt,
}

impl SequenceMessageThemeEmission {
    pub(crate) fn from_terminal_writer(
        typed_stroke: Option<&str>,
        stroke_overridden: bool,
        receipt: SequenceMessageThemeReceipt,
    ) -> Self {
        let has_lines = receipt.line_candidates != 0;
        let complete_line_emission = has_lines && receipt.emitted_lines == receipt.line_candidates;
        Self {
            stroke_emitted: complete_line_emission && typed_stroke.is_some(),
            stroke_overridden: has_lines && stroke_overridden,
            receipt,
        }
    }
}

/// Winner and terminal-emission facts produced by a Sequence static rectangle writer.
///
/// This receipt is deliberately narrower than the Actor receipt: the current direct tranche owns
/// only unqualified static fill and stroke for Note and Activation surfaces. Each writer records
/// the surfaces it actually emits so evidence cannot be manufactured from the compiled route
/// matrix alone.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceStaticRectThemeReceipt {
    static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    emitted_rects: usize,
}

impl SequenceStaticRectThemeReceipt {
    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
    }

    pub(crate) fn record_rect_emission(&mut self) {
        self.emitted_rects = self.emitted_rects.saturating_add(1);
    }

    fn merge(&mut self, other: Self) {
        self.static_winners.extend(other.static_winners);
        self.emitted_rects = self.emitted_rects.max(other.emitted_rects);
    }

    fn route_won(
        &self,
        rule_index: usize,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
            && self
                .static_winners
                .contains(&(rule_index, style_property_for_facet(facet)))
    }
}

/// Complete terminal emission facts for one Sequence static rectangle paint tranche.
#[derive(Debug, Clone, Default)]
struct SequenceStaticRectThemeState {
    surface_count: usize,
    fill_emitted: bool,
    fill_overridden: bool,
    stroke_emitted: bool,
    stroke_overridden: bool,
    receipt: SequenceStaticRectThemeReceipt,
}

impl SequenceStaticRectThemeState {
    fn merge(&mut self, emission: SequenceStaticRectThemeEmission) {
        self.surface_count = self.surface_count.max(emission.surface_count);
        self.fill_emitted |= emission.fill_emitted;
        self.fill_overridden |= emission.fill_overridden;
        self.stroke_emitted |= emission.stroke_emitted;
        self.stroke_overridden |= emission.stroke_overridden;
        self.receipt.merge(emission.receipt);
    }
}

/// Complete writer-owned emission facts shared by Note and Activation rectangles.
#[derive(Debug)]
pub(crate) struct SequenceStaticRectThemeEmission {
    surface_count: usize,
    fill_emitted: bool,
    fill_overridden: bool,
    stroke_emitted: bool,
    stroke_overridden: bool,
    receipt: SequenceStaticRectThemeReceipt,
}

impl SequenceStaticRectThemeEmission {
    pub(crate) fn from_terminal_writer(
        surface_count: usize,
        typed_fill: Option<&str>,
        fill_overridden: bool,
        typed_stroke: Option<&str>,
        stroke_overridden: bool,
        receipt: SequenceStaticRectThemeReceipt,
    ) -> Self {
        let has_surfaces = surface_count != 0;
        let complete_rect_emission = has_surfaces && receipt.emitted_rects == surface_count;
        Self {
            surface_count,
            fill_emitted: complete_rect_emission && typed_fill.is_some(),
            fill_overridden: has_surfaces && fill_overridden,
            stroke_emitted: complete_rect_emission && typed_stroke.is_some(),
            stroke_overridden: has_surfaces && stroke_overridden,
            receipt,
        }
    }
}

#[derive(Debug, Clone)]
struct SequenceRoleTypographyReceipt {
    static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    config_overrides: BTreeSet<crate::diagram_theme::ThemeTypographyProperty>,
    label_candidates: Cell<usize>,
    emitted_labels: Cell<usize>,
}

impl SequenceRoleTypographyReceipt {
    fn from_resolved(typography: &crate::sequence::SequenceResolvedTypography) -> Self {
        let mut static_winners = BTreeSet::new();
        if let Some(style) = typography.resolved_style() {
            record_style_winners(&mut static_winners, style);
        }
        Self {
            static_winners,
            config_overrides: typography.config_overrides().clone(),
            label_candidates: Cell::new(0),
            emitted_labels: Cell::new(0),
        }
    }

    fn record_candidate(&self) {
        self.label_candidates
            .set(self.label_candidates.get().saturating_add(1));
    }

    fn record_emission(&self) {
        self.emitted_labels
            .set(self.emitted_labels.get().saturating_add(1));
    }

    fn route_won(
        &self,
        rule_index: usize,
        _selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        self.static_winners
            .contains(&(rule_index, style_property_for_facet(facet)))
    }

    fn complete(&self) -> bool {
        self.label_candidates.get() != 0 && self.label_candidates.get() == self.emitted_labels.get()
    }
}

/// One operation-local receipt shared by every Sequence role-label terminal writer.
#[derive(Debug, Clone)]
pub(crate) struct SequenceTypographyThemeReceipt {
    actor: SequenceRoleTypographyReceipt,
    message: SequenceRoleTypographyReceipt,
    note: SequenceRoleTypographyReceipt,
    loop_label: SequenceRoleTypographyReceipt,
}

impl SequenceTypographyThemeReceipt {
    pub(crate) fn from_plan(plan: &crate::sequence::SequenceTypographyPlan) -> Self {
        Self {
            actor: SequenceRoleTypographyReceipt::from_resolved(plan.actor()),
            message: SequenceRoleTypographyReceipt::from_resolved(plan.message()),
            note: SequenceRoleTypographyReceipt::from_resolved(plan.note()),
            loop_label: SequenceRoleTypographyReceipt::from_resolved(plan.loop_label()),
        }
    }

    fn role(
        &self,
        role: crate::sequence::SequenceTypographyRole,
    ) -> &SequenceRoleTypographyReceipt {
        match role {
            crate::sequence::SequenceTypographyRole::Actor => &self.actor,
            crate::sequence::SequenceTypographyRole::Message => &self.message,
            crate::sequence::SequenceTypographyRole::Note => &self.note,
            crate::sequence::SequenceTypographyRole::Loop => &self.loop_label,
        }
    }

    pub(crate) fn record_candidate(&self, role: crate::sequence::SequenceTypographyRole) {
        self.role(role).record_candidate();
    }

    pub(crate) fn record_emission(&self, role: crate::sequence::SequenceTypographyRole) {
        self.role(role).record_emission();
    }

    pub(crate) fn record_terminal_text(&self, role: crate::sequence::SequenceTypographyRole) {
        self.record_candidate(role);
        self.record_emission(role);
    }
}

#[derive(Debug, Default)]
struct SequenceRuleObservation {
    applicable: bool,
    incomplete: bool,
    capabilities: BTreeSet<ThemeCapability>,
    residual: Option<FamilyThemeResidualReason>,
}

fn observe_static_rect_rule(
    surface: &SequenceStaticRectThemeState,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if surface.surface_count == 0 || !surface.receipt.route_won(rule_index, selector, facet) {
        return;
    }
    observation.applicable = true;
    match facet {
        FamilyThemeRuleFacet::Fill(_) if surface.fill_overridden => return,
        FamilyThemeRuleFacet::Stroke(_) if surface.stroke_overridden => return,
        _ => {}
    }
    match (disposition, facet) {
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Fill(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !surface.fill_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Stroke(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !surface.stroke_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

fn observe_message_rule(
    message: &SequenceMessageThemeState,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if message.receipt.line_candidates == 0
        || !message.receipt.route_won(rule_index, selector, facet)
    {
        return;
    }
    observation.applicable = true;
    if message.stroke_overridden
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
        )
    {
        return;
    }
    match (disposition, facet) {
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Stroke(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !message.stroke_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

fn observe_lifeline_rule(
    lifeline: &SequenceLifelineThemeState,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if lifeline.receipt.line_candidates == 0
        || !lifeline.receipt.route_won(rule_index, selector, facet)
    {
        return;
    }
    observation.applicable = true;
    if matches!(
        facet,
        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
    ) {
        if lifeline.paint_overridden {
            return;
        }
        if lifeline.selected_property != Some(style_property_for_facet(facet)) {
            return;
        }
    }
    match (disposition, facet) {
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Fill(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            )
            | FamilyThemeRuleFacet::Stroke(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !lifeline.paint_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::StrokeWidth) => {
            if !lifeline.stroke_width_emitted {
                observation.incomplete = true;
                return;
            }
            observation
                .capabilities
                .insert(ThemeCapability::BorderStyling);
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

fn observe_typography_rule(
    role: &SequenceRoleTypographyReceipt,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if role.label_candidates.get() == 0 {
        return;
    }
    if !role.route_won(rule_index, selector, facet) {
        if matches!(
            (disposition, selector),
            (
                FamilyThemeDisposition::Unsupported,
                FamilyThemeSelectorShape::Ordinal { .. }
            )
        ) {
            observation.applicable = true;
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        return;
    }
    observation.applicable = true;
    if let FamilyThemeRuleFacet::Typography(property) = facet
        && role.config_overrides.contains(&property)
    {
        return;
    }
    match (disposition, facet) {
        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Typography(_)) => {
            if role.complete() {
                observation.capabilities.insert(ThemeCapability::Typography);
            } else {
                observation.incomplete = true;
            }
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

fn capability_for_paint(kind: FamilyThemePaintKind) -> ThemeCapability {
    match kind {
        FamilyThemePaintKind::Transparent => ThemeCapability::TransparentPaint,
        FamilyThemePaintKind::Solid => ThemeCapability::SolidPaint,
        FamilyThemePaintKind::Clear
        | FamilyThemePaintKind::LinearGradient
        | FamilyThemePaintKind::RadialGradient
        | FamilyThemePaintKind::Pattern => {
            unreachable!("only typed scalar paints reach direct Sequence paint writers")
        }
    }
}

/// Records actual Sequence writer emission so program routes alone cannot manufacture evidence.
#[derive(Debug, Default)]
pub(crate) struct SequenceThemeEvidenceRecorder {
    state: Mutex<SequenceThemeEvidenceState>,
}

impl SequenceThemeEvidenceRecorder {
    pub(crate) fn record_actor_emission(
        &self,
        actor_count: usize,
        actor_fill_emitted: bool,
        actor_fill_unhandled: bool,
        actor_fill_overridden: bool,
        actor_stroke_emitted: bool,
        actor_stroke_unhandled: bool,
        actor_stroke_overridden: bool,
        actor_style_receipt: SequenceActorThemeReceipt,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.actor_count = state.actor_count.max(actor_count);
        state.actor_fill_emitted |= actor_count != 0 && actor_fill_emitted;
        state.actor_fill_unhandled |= actor_count != 0 && actor_fill_unhandled;
        state.actor_fill_overridden |= actor_count != 0 && actor_fill_overridden;
        state.actor_stroke_emitted |= actor_count != 0 && actor_stroke_emitted;
        state.actor_stroke_unhandled |= actor_count != 0 && actor_stroke_unhandled;
        state.actor_stroke_overridden |= actor_count != 0 && actor_stroke_overridden;
        state.actor_style_receipt.merge(actor_style_receipt);
    }

    pub(crate) fn record_note_emission(&self, emission: SequenceStaticRectThemeEmission) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.note.merge(emission);
    }

    pub(crate) fn record_lifeline_emission(&self, emission: SequenceLifelineThemeEmission) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.lifeline.merge(emission);
    }

    pub(crate) fn record_message_emission(&self, emission: SequenceMessageThemeEmission) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.message.merge(emission);
    }

    pub(crate) fn record_activation_emission(&self, emission: SequenceStaticRectThemeEmission) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.activation.merge(emission);
    }

    pub(crate) fn record_typography_emission(&self, receipt: SequenceTypographyThemeReceipt) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        debug_assert!(
            state.typography.is_none(),
            "Sequence role typography is sealed once per terminal SVG"
        );
        state.typography = Some(receipt);
    }

    pub(crate) fn finish(&self, theme: Option<&ResolvedDiagramTheme>) -> FamilyThemeEvidence {
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Some(theme) = theme else {
            return evidence;
        };

        let mut actor_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut lifeline_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut message_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut note_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut activation_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut typography_rules =
            BTreeMap::<ThemeTarget, BTreeMap<usize, SequenceRuleObservation>>::new();

        // Seed every Actor rule plus every static unqualified Message signal paint and directly
        // owned Note/Activation rule before checking winners.
        // This keeps superseded rules, unmatched ordinal selectors, and empty terminal models in
        // the ledger so the final pass can classify them as explicitly not applicable instead of
        // leaving required keys unaccounted.
        for route in theme.family_mechanism_routes().iter().copied() {
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Actor,
                ..
            } = route.mechanism()
            {
                actor_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Lifeline,
                selector: FamilyThemeSelectorShape::Static { variant: None },
                facet:
                    FamilyThemeRuleFacet::Fill(_)
                    | FamilyThemeRuleFacet::Stroke(_)
                    | FamilyThemeRuleFacet::StrokeWidth,
            } = route.mechanism()
            {
                lifeline_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Message,
                selector: FamilyThemeSelectorShape::Static { variant: None },
                facet: FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_),
            } = route.mechanism()
            {
                message_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Note,
                selector: FamilyThemeSelectorShape::Static { variant: None },
                ..
            } = route.mechanism()
            {
                note_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Activation,
                selector: FamilyThemeSelectorShape::Static { variant: None },
                ..
            } = route.mechanism()
            {
                activation_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index, target, ..
            } = route.mechanism()
                && crate::sequence::SequenceTypographyRole::from_target(target).is_some()
            {
                typography_rules
                    .entry(target)
                    .or_default()
                    .entry(rule_index)
                    .or_default();
            }
        }

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(_) => {
                    let key = theme.family_mechanism_key(route);
                    match route.disposition() {
                        FamilyThemeDisposition::Unsupported => {
                            if state.actor_count == 0 {
                                evidence.mark_not_applicable(key);
                            } else {
                                evidence.mark_residual(
                                    key,
                                    FamilyThemeResidualReason::UnsupportedTypography,
                                );
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter => {
                            // No Sequence base-typography route is direct yet. Leaving a future
                            // route unaccounted fails closed instead of guessing consumption.
                        }
                        FamilyThemeDisposition::LegacyCompatibility => {}
                    }
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target:
                        target @ (ThemeTarget::ActorLabel
                        | ThemeTarget::MessageLabel
                        | ThemeTarget::NoteLabel
                        | ThemeTarget::LoopLabel),
                    selector,
                    facet,
                } => {
                    let Some(role) = crate::sequence::SequenceTypographyRole::from_target(target)
                    else {
                        continue;
                    };
                    let Some(receipt) = state.typography.as_ref().map(|receipt| receipt.role(role))
                    else {
                        continue;
                    };
                    let Some(observation) = typography_rules
                        .get_mut(&target)
                        .and_then(|rules| rules.get_mut(&rule_index))
                    else {
                        continue;
                    };
                    observe_typography_rule(
                        receipt,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Lifeline,
                    selector,
                    facet,
                } if matches!(selector, FamilyThemeSelectorShape::Static { variant: None }) => {
                    let Some(observation) = lifeline_rules.get_mut(&rule_index) else {
                        continue;
                    };
                    observe_lifeline_rule(
                        &state.lifeline,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Message,
                    selector,
                    facet,
                } if matches!(selector, FamilyThemeSelectorShape::Static { variant: None }) => {
                    let Some(observation) = message_rules.get_mut(&rule_index) else {
                        continue;
                    };
                    observe_message_rule(
                        &state.message,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Actor,
                    selector,
                    facet,
                } => {
                    let observation = actor_rules.entry(rule_index).or_default();
                    if state.actor_count == 0
                        || !state
                            .actor_style_receipt
                            .route_won(rule_index, selector, facet)
                    {
                        continue;
                    }
                    observation.applicable = true;
                    if state.actor_fill_overridden && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                    {
                        continue;
                    }
                    if state.actor_stroke_overridden
                        && matches!(facet, FamilyThemeRuleFacet::Stroke(_))
                    {
                        continue;
                    }
                    match route.disposition() {
                        FamilyThemeDisposition::TypedAdapter
                            if matches!(
                                facet,
                                FamilyThemeRuleFacet::Fill(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                                )
                            ) =>
                        {
                            if state.actor_fill_unhandled {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                continue;
                            }
                            if !state.actor_fill_emitted {
                                continue;
                            }
                            match facet {
                                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Transparent) => {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::TransparentPaint);
                                }
                                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid) => {
                                    observation.capabilities.insert(ThemeCapability::SolidPaint);
                                }
                                _ => {
                                    observation
                                        .residual
                                        .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                }
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter
                            if matches!(
                                facet,
                                FamilyThemeRuleFacet::Stroke(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                                )
                            ) =>
                        {
                            if state.actor_stroke_unhandled {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                continue;
                            }
                            if !state.actor_stroke_emitted {
                                continue;
                            }
                            match facet {
                                FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Transparent) => {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::TransparentPaint);
                                }
                                FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Solid) => {
                                    observation.capabilities.insert(ThemeCapability::SolidPaint);
                                }
                                _ => {
                                    observation
                                        .residual
                                        .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                }
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter => observation.incomplete = true,
                        FamilyThemeDisposition::Unsupported => {
                            observation
                                .residual
                                .get_or_insert(unsupported_reason_for_facet(facet));
                        }
                        FamilyThemeDisposition::LegacyCompatibility => {}
                    }
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: target @ (ThemeTarget::Note | ThemeTarget::Activation),
                    selector,
                    facet,
                } if matches!(selector, FamilyThemeSelectorShape::Static { variant: None }) => {
                    let (surface, observation) = match target {
                        ThemeTarget::Note => {
                            (&state.note, note_rules.entry(rule_index).or_default())
                        }
                        ThemeTarget::Activation => (
                            &state.activation,
                            activation_rules.entry(rule_index).or_default(),
                        ),
                        _ => unreachable!("guarded by the static rectangle target pattern"),
                    };
                    observe_static_rect_rule(
                        surface,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Actor,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if state.actor_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if matches!(route.disposition(), FamilyThemeDisposition::Unsupported) {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Actor,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if state.actor_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if !matches!(
                        route.disposition(),
                        FamilyThemeDisposition::LegacyCompatibility
                    ) {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {
                    // This narrow adapter owns Actor, unqualified static Lifeline paint, Message
                    // stroke, and Note/Activation paint evidence. Message fill is observed only to
                    // account for its participation in the shared signalColor projection. Other
                    // Sequence targets and selector classes remain unaccounted unless compatibility
                    // rejects them.
                }
            }
        }

        for (rule_index, observation) in actor_rules {
            let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Actor,
            };
            if state.actor_count == 0 || !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A future direct facet reached the actor but has no terminal receipt yet.
            } else if observation.capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(key, observation.capabilities);
            }
        }
        finish_lifeline_rules(&mut evidence, &state.lifeline, lifeline_rules);
        finish_message_rules(&mut evidence, &state.message, message_rules);
        finish_static_rect_rules(&mut evidence, ThemeTarget::Note, &state.note, note_rules);
        finish_static_rect_rules(
            &mut evidence,
            ThemeTarget::Activation,
            &state.activation,
            activation_rules,
        );
        finish_typography_rules(&mut evidence, state.typography.as_ref(), typography_rules);
        evidence
    }
}

fn finish_typography_rules(
    evidence: &mut FamilyThemeEvidence,
    receipt: Option<&SequenceTypographyThemeReceipt>,
    rules_by_target: BTreeMap<ThemeTarget, BTreeMap<usize, SequenceRuleObservation>>,
) {
    for (target, rules) in rules_by_target {
        let role = crate::sequence::SequenceTypographyRole::from_target(target)
            .expect("Sequence typography rules are keyed only by role targets");
        let candidate_count =
            receipt.map_or(0, |receipt| receipt.role(role).label_candidates.get());
        for (rule_index, observation) in rules {
            let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            if candidate_count == 0 || !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // The rule reached a terminal label candidate without a complete writer seal.
            } else if observation.capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(key, observation.capabilities);
            }
        }
    }
}

fn finish_lifeline_rules(
    evidence: &mut FamilyThemeEvidence,
    lifeline: &SequenceLifelineThemeState,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target: ThemeTarget::Lifeline,
        };
        if lifeline.receipt.line_candidates == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {
            // A selected Lifeline paint reached a candidate without a complete actor-line seal.
        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

fn finish_message_rules(
    evidence: &mut FamilyThemeEvidence,
    message: &SequenceMessageThemeState,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target: ThemeTarget::Message,
        };
        if message.receipt.line_candidates == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {
            // A direct Message stroke reached a terminal candidate without a complete line seal.
        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

fn finish_static_rect_rules(
    evidence: &mut FamilyThemeEvidence,
    target: ThemeTarget,
    surface: &SequenceStaticRectThemeState,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target,
        };
        if surface.surface_count == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {
            // A direct facet reached a rectangle but the terminal writer did not seal it.
        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

fn record_style_winners(
    winners: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
    style: &ResolvedThemeStyle,
) {
    winners.extend(
        style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property)),
    );
}

const fn style_property_for_facet(facet: FamilyThemeRuleFacet) -> ResolvedStyleProperty {
    match facet {
        FamilyThemeRuleFacet::Fill(_) => ResolvedStyleProperty::Fill,
        FamilyThemeRuleFacet::Stroke(_) => ResolvedStyleProperty::Stroke,
        FamilyThemeRuleFacet::StrokeWidth => ResolvedStyleProperty::StrokeWidth,
        FamilyThemeRuleFacet::StrokeDasharray => ResolvedStyleProperty::StrokeDasharray,
        FamilyThemeRuleFacet::StrokeLinecap => ResolvedStyleProperty::StrokeLinecap,
        FamilyThemeRuleFacet::StrokeLinejoin => ResolvedStyleProperty::StrokeLinejoin,
        FamilyThemeRuleFacet::Opacity => ResolvedStyleProperty::Opacity,
        FamilyThemeRuleFacet::FillOpacity => ResolvedStyleProperty::FillOpacity,
        FamilyThemeRuleFacet::StrokeOpacity => ResolvedStyleProperty::StrokeOpacity,
        FamilyThemeRuleFacet::Radius => ResolvedStyleProperty::Radius,
        FamilyThemeRuleFacet::Padding => ResolvedStyleProperty::Padding,
        FamilyThemeRuleFacet::Typography(property) => ResolvedStyleProperty::Typography(property),
        FamilyThemeRuleFacet::Effect => ResolvedStyleProperty::Effect,
    }
}

fn unsupported_reason_for_facet(facet: FamilyThemeRuleFacet) -> FamilyThemeResidualReason {
    match facet {
        FamilyThemeRuleFacet::Typography(_) => FamilyThemeResidualReason::UnsupportedTypography,
        FamilyThemeRuleFacet::Effect => FamilyThemeResidualReason::UnsupportedEffect,
        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_) => {
            FamilyThemeResidualReason::UnsupportedPaint
        }
        FamilyThemeRuleFacet::StrokeWidth
        | FamilyThemeRuleFacet::StrokeDasharray
        | FamilyThemeRuleFacet::StrokeLinecap
        | FamilyThemeRuleFacet::StrokeLinejoin
        | FamilyThemeRuleFacet::Opacity
        | FamilyThemeRuleFacet::FillOpacity
        | FamilyThemeRuleFacet::StrokeOpacity
        | FamilyThemeRuleFacet::Radius
        | FamilyThemeRuleFacet::Padding => FamilyThemeResidualReason::UnsupportedGeometry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, Specified, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };

    #[test]
    fn actor_rule_is_not_applicable_when_terminal_model_has_no_actors() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence actor theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        let evidence = SequenceThemeEvidenceRecorder::default().finish(Some(&resolved));

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            [crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn note_winner_without_complete_rect_receipt_remains_incomplete() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Note,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Note theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        for (note_count, emitted_rects) in [(1, 0), (2, 1)] {
            let mut receipt = SequenceStaticRectThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Note,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..emitted_rects {
                receipt.record_rect_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_note_emission(SequenceStaticRectThemeEmission::from_terminal_writer(
                note_count,
                Some("#ef4444"),
                false,
                None,
                false,
                receipt,
            ));

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn message_winner_without_complete_line_receipt_remains_incomplete() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Message,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Message theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        for (line_candidates, emitted_lines) in [(1, 0), (2, 1)] {
            let mut receipt = SequenceMessageThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Message,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..line_candidates {
                receipt.record_line_candidate();
            }
            for _ in 0..emitted_lines {
                receipt.record_line_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_message_emission(SequenceMessageThemeEmission::from_terminal_writer(
                Some("#2563eb"),
                false,
                receipt,
            ));

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn lifeline_winner_without_complete_actor_line_receipt_remains_incomplete() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Lifeline,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Lifeline theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        for (line_candidates, emitted_lines) in [(1, 0), (2, 1)] {
            let mut receipt = SequenceLifelineThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Lifeline,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..line_candidates {
                receipt.record_line_candidate();
            }
            for _ in 0..emitted_lines {
                receipt.record_line_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_lifeline_emission(SequenceLifelineThemeEmission::from_terminal_writer(
                Some("#2563eb"),
                None,
                Some(ResolvedStyleProperty::Stroke),
                false,
                receipt,
            ));

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn lifeline_stroke_width_requires_complete_actor_line_receipt() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Lifeline,
                            ThemeStylePatch::default()
                                .with_stroke_width(2.0)
                                .expect("valid Lifeline stroke width"),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Lifeline width theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);

        for (line_candidates, emitted_lines, paint_overridden, expected_applied) in [
            (1, 0, false, false),
            (2, 1, false, false),
            (2, 2, true, true),
        ] {
            let mut receipt = SequenceLifelineThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Lifeline,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..line_candidates {
                receipt.record_line_candidate();
            }
            for _ in 0..emitted_lines {
                receipt.record_line_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_lifeline_emission(SequenceLifelineThemeEmission::from_terminal_writer(
                None,
                Some(2.0),
                None,
                paint_overridden,
                receipt,
            ));

            let evidence = recorder.finish(Some(&resolved));
            assert_eq!(evidence.applied().len(), usize::from(expected_applied));
            assert_eq!(
                evidence.applied_capabilities(),
                if expected_applied {
                    BTreeSet::from([ThemeCapability::BorderStyling])
                } else {
                    BTreeSet::new()
                }
            );
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn lifeline_mixed_typed_paint_and_unsupported_sibling_facets_remains_residual() {
        let mut style = ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#2563eb").expect("valid Lifeline stroke"));
        style.geometry.radius = Specified::Value(6.0);
        style.spacing.padding = Specified::Value(crate::diagram_theme::InsetsPx::all(4.0));
        style.paint.opacity = Specified::Value(0.75);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(ThemeTarget::Lifeline, style)
                            .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile mixed Sequence Lifeline theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        let mut receipt = SequenceLifelineThemeReceipt::default();
        receipt.record_static_style(&resolved.style(
            ThemeTarget::Lifeline,
            ThemeVariant::Default,
            None,
        ));
        receipt.record_line_candidate();
        receipt.record_line_emission();
        let recorder = SequenceThemeEvidenceRecorder::default();
        recorder.record_lifeline_emission(SequenceLifelineThemeEmission::from_terminal_writer(
            Some("#2563eb"),
            None,
            Some(ResolvedStyleProperty::Stroke),
            false,
            receipt,
        ));

        let evidence = recorder.finish(Some(&resolved));
        assert!(evidence.applied().is_empty());
        let [residual] = evidence.residuals() else {
            panic!("mixed Lifeline rule must retain exactly one residual")
        };
        assert_eq!(
            residual.key(),
            &crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Lifeline,
            }
        );
        assert_eq!(
            residual.reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );
    }

    #[test]
    fn activation_winner_without_complete_rect_receipt_remains_incomplete() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Activation,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Activation theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
        for (activation_count, emitted_rects) in [(1, 0), (2, 1)] {
            let mut receipt = SequenceStaticRectThemeReceipt::default();
            receipt.record_static_style(&resolved.style(
                ThemeTarget::Activation,
                ThemeVariant::Default,
                None,
            ));
            for _ in 0..emitted_rects {
                receipt.record_rect_emission();
            }
            let recorder = SequenceThemeEvidenceRecorder::default();
            recorder.record_activation_emission(
                SequenceStaticRectThemeEmission::from_terminal_writer(
                    activation_count,
                    Some("#ef4444"),
                    false,
                    None,
                    false,
                    receipt,
                ),
            );

            let evidence = recorder.finish(Some(&resolved));
            assert!(evidence.applied().is_empty());
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }
}
