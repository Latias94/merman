use std::cell::{Cell, RefCell};
#[cfg(merman_internal_theme_acceptance)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::diagram_theme::{
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedStyleProperty, ResolvedThemeStyle,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::resolved_style_property_for_facet as style_property_for_facet;

#[derive(Debug, Clone, Default)]
pub(super) struct SequenceThemeEvidenceState {
    pub(super) actor_count: usize,
    pub(super) actor_fill_emitted: bool,
    pub(super) actor_fill_unhandled: bool,
    pub(super) actor_fill_overridden: bool,
    pub(super) actor_stroke_emitted: bool,
    pub(super) actor_stroke_unhandled: bool,
    pub(super) actor_stroke_overridden: bool,
    pub(super) actor_style_receipt: SequenceActorThemeReceipt,
    pub(super) lifeline: SequenceLifelineThemeState,
    pub(super) message: SequenceMessageThemeState,
    pub(super) sequence_number: SequenceNumberLabelThemeState,
    pub(super) keyword_background: SequenceControlThemeState,
    pub(super) loop_frame: SequenceControlThemeState,
    pub(super) note: SequenceStaticRectThemeState,
    pub(super) activation: SequenceStaticRectThemeState,
    pub(super) typography: Option<SequenceTypographyThemeReceipt>,
    pub(super) unsupported_text: Option<crate::family::FamilyThemeEvidence>,
}

/// Winner facts produced by the terminal Sequence Actor writer.
///
/// Static and ordinal winners remain separate because a static rule is resolved against the
/// family default while ordinal selectors are evaluated against each concrete one-based actor
/// ordinal. The writer resolves each ordinal exactly once and records every facet winner from
/// that result; evidence finalization only consumes these facts and performs no selector scan.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceActorThemeReceipt {
    pub(super) static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    pub(super) ordinal_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    geometry_requested: bool,
    pub(crate) effect_requested: bool,
    pub(crate) effect_binding_used: bool,
    pub(crate) effect_cleared: bool,
    pub(crate) effect_unhandled: bool,
    effect_rects: Cell<usize>,
    geometry_candidates: Cell<usize>,
    geometry_rects: Cell<usize>,
    pub(super) geometry_unhandled: Cell<bool>,
    pub(super) stroke_width_overridden: bool,
}

impl SequenceActorThemeReceipt {
    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
        self.geometry_requested = style.radius_resolution().winner().is_some()
            || style.stroke_width_resolution().winner().is_some();
    }

    pub(crate) fn record_stroke_width_override(&mut self, overridden: bool) {
        self.stroke_width_overridden = overridden;
    }

    pub(crate) fn record_geometry_candidate(&self) {
        if self.geometry_requested || self.effect_requested {
            self.geometry_candidates
                .set(self.geometry_candidates.get().saturating_add(1));
        }
    }

    pub(crate) fn record_unhandled_geometry(&self) {
        if self.geometry_requested || self.effect_requested {
            self.geometry_unhandled.set(true);
        }
    }

    pub(crate) fn record_geometry_rect(&self) {
        if self.geometry_requested {
            self.geometry_rects
                .set(self.geometry_rects.get().saturating_add(1));
        }
    }

    pub(crate) fn record_effect_rect(&self) {
        if self.effect_requested {
            self.effect_rects
                .set(self.effect_rects.get().saturating_add(1));
        }
    }

    pub(super) fn effect_complete(&self) -> bool {
        self.geometry_candidates.get() != 0
            && self.geometry_candidates.get() == self.effect_rects.get()
    }

    pub(super) fn geometry_complete(&self) -> bool {
        self.geometry_candidates.get() != 0
            && self.geometry_candidates.get() == self.geometry_rects.get()
    }

    pub(crate) fn record_ordinal_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.ordinal_winners, style);
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.static_winners.extend(other.static_winners);
        self.ordinal_winners.extend(other.ordinal_winners);
        self.geometry_requested |= other.geometry_requested;
        self.effect_requested |= other.effect_requested;
        self.effect_binding_used |= other.effect_binding_used;
        self.effect_cleared |= other.effect_cleared;
        self.effect_unhandled |= other.effect_unhandled;
        self.effect_rects
            .set(self.effect_rects.get().max(other.effect_rects.get()));
        self.geometry_candidates.set(
            self.geometry_candidates
                .get()
                .max(other.geometry_candidates.get()),
        );
        self.geometry_rects
            .set(self.geometry_rects.get().max(other.geometry_rects.get()));
        self.geometry_unhandled
            .set(self.geometry_unhandled.get() || other.geometry_unhandled.get());
        self.stroke_width_overridden |= other.stroke_width_overridden;
    }

    pub(super) fn route_won(
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
struct SequenceLineThemeReceipt {
    static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    #[cfg(merman_internal_theme_acceptance)]
    static_selectors:
        BTreeMap<ResolvedStyleProperty, crate::theme_route_cutover::ThemeRouteCutoverSelector>,
    line_candidates: usize,
    emitted_lines: usize,
}

impl SequenceLineThemeReceipt {
    fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
        #[cfg(merman_internal_theme_acceptance)]
        {
            for (property, origin) in [
                (
                    ResolvedStyleProperty::Fill,
                    style.fill_resolution().winner(),
                ),
                (
                    ResolvedStyleProperty::Stroke,
                    style.stroke_resolution().winner(),
                ),
                (
                    ResolvedStyleProperty::StrokeWidth,
                    style.stroke_width_resolution().winner(),
                ),
            ] {
                if let Some(origin) = origin {
                    self.static_selectors.insert(
                        property,
                        match origin.variant() {
                            None => crate::theme_route_cutover::ThemeRouteCutoverSelector::
                                StaticUnqualified,
                            Some(variant) =>
                                crate::theme_route_cutover::ThemeRouteCutoverSelector::
                                    StaticVariant(variant),
                        },
                    );
                }
            }
        }
    }

    fn record_line_candidate(&mut self) {
        self.line_candidates = self.line_candidates.saturating_add(1);
    }

    fn record_line_emission(&mut self) {
        self.emitted_lines = self.emitted_lines.saturating_add(1);
    }

    fn merge(&mut self, other: Self) {
        self.static_winners.extend(other.static_winners);
        #[cfg(merman_internal_theme_acceptance)]
        self.static_selectors.extend(other.static_selectors);
        self.line_candidates = self.line_candidates.max(other.line_candidates);
        self.emitted_lines = self.emitted_lines.max(other.emitted_lines);
    }

    fn route_won(&self, rule_index: usize, facet: FamilyThemeRuleFacet) -> bool {
        self.static_winners
            .contains(&(rule_index, style_property_for_facet(facet)))
    }

    fn candidate_count(&self) -> usize {
        self.line_candidates
    }

    fn has_complete_emission(&self) -> bool {
        self.line_candidates != 0 && self.emitted_lines == self.line_candidates
    }

    #[cfg(merman_internal_theme_acceptance)]
    fn selector_for(
        &self,
        property: ResolvedStyleProperty,
    ) -> Option<crate::theme_route_cutover::ThemeRouteCutoverSelector> {
        self.static_selectors.get(&property).copied()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceLifelineThemeReceipt {
    line: SequenceLineThemeReceipt,
    pub(crate) effect_requested: bool,
    pub(crate) effect_binding_used: bool,
    pub(crate) effect_cleared: bool,
    pub(crate) effect_unhandled: bool,
    emitted_effects: usize,
    #[cfg(merman_internal_theme_acceptance)]
    raster_terminals: BTreeMap<String, crate::theme_raster_paint::ThemeRasterPaintTerminal>,
    #[cfg(merman_internal_theme_acceptance)]
    raster_terminals_invalid: bool,
}

impl SequenceLifelineThemeReceipt {
    pub(crate) fn record_effect_emission(&mut self) {
        self.emitted_effects = self.emitted_effects.saturating_add(1);
    }

    pub(super) fn effect_complete(&self) -> bool {
        self.line.has_complete_emission() && self.candidate_count() == self.emitted_effects
    }

    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        self.line.record_static_style(style);
    }

    pub(crate) fn record_line_candidate(&mut self) {
        self.line.record_line_candidate();
    }

    pub(crate) fn record_line_emission(
        &mut self,
        actor_index: usize,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke_width: f64,
    ) {
        self.record_line_emission_with_effective_width(
            actor_index,
            x1,
            y1,
            x2,
            y2,
            stroke_width,
            stroke_width,
        );
    }

    pub(crate) fn record_line_emission_with_effective_width(
        &mut self,
        actor_index: usize,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        authored_stroke_width: f64,
        effective_stroke_width: f64,
    ) {
        self.line.record_line_emission();
        #[cfg(not(merman_internal_theme_acceptance))]
        let _ = (
            actor_index,
            x1,
            y1,
            x2,
            y2,
            authored_stroke_width,
            effective_stroke_width,
        );
        #[cfg(merman_internal_theme_acceptance)]
        {
            let terminal_id = format!("actor{actor_index}");
            let Some(terminal) =
                crate::theme_raster_paint::ThemeRasterPaintTerminal::sequence_lifeline_with_effective_width(
                    terminal_id.clone(),
                    crate::theme_raster_paint::ThemeRasterPaintBinding::FillAndStrokeFromStroke,
                    x1,
                    y1,
                    x2,
                    y2,
                    authored_stroke_width,
                    effective_stroke_width,
                )
            else {
                self.raster_terminals_invalid = true;
                return;
            };
            if self
                .raster_terminals
                .insert(terminal_id, terminal)
                .is_some()
            {
                self.raster_terminals_invalid = true;
            }
        }
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.line.merge(other.line);
        self.effect_requested |= other.effect_requested;
        self.effect_binding_used |= other.effect_binding_used;
        self.effect_cleared |= other.effect_cleared;
        self.effect_unhandled |= other.effect_unhandled;
        self.emitted_effects = self.emitted_effects.max(other.emitted_effects);
        #[cfg(merman_internal_theme_acceptance)]
        {
            self.raster_terminals_invalid |= other.raster_terminals_invalid
                || other
                    .raster_terminals
                    .keys()
                    .any(|terminal_id| self.raster_terminals.contains_key(terminal_id));
            self.raster_terminals.extend(other.raster_terminals);
        }
    }

    pub(super) fn route_won(
        &self,
        rule_index: usize,
        _selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        self.line.route_won(rule_index, facet)
    }

    pub(super) fn candidate_count(&self) -> usize {
        self.line.candidate_count()
    }

    fn has_complete_emission(&self) -> bool {
        self.line.has_complete_emission()
    }

    #[cfg(merman_internal_theme_acceptance)]
    pub(super) fn raster_paint_terminals(
        &self,
    ) -> Option<Vec<crate::theme_raster_paint::ThemeRasterPaintTerminal>> {
        (self.has_complete_emission()
            && !self.raster_terminals_invalid
            && self.raster_terminals.len() == self.line.emitted_lines)
            .then(|| self.raster_terminals.values().cloned().collect())
    }

    #[cfg(merman_internal_theme_acceptance)]
    pub(super) fn raster_paint_selector(
        &self,
        property: ResolvedStyleProperty,
    ) -> Option<crate::theme_route_cutover::ThemeRouteCutoverSelector> {
        self.line.selector_for(property)
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct SequenceLifelineThemeState {
    pub(super) paint_emitted: bool,
    pub(super) stroke_width_emitted: bool,
    pub(super) paint_overridden: bool,
    pub(super) selected_property: Option<ResolvedStyleProperty>,
    pub(super) receipt: SequenceLifelineThemeReceipt,
}

impl SequenceLifelineThemeState {
    pub(super) fn merge(&mut self, emission: SequenceLifelineThemeEmission) {
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
    pub(super) paint_emitted: bool,
    pub(super) stroke_width_emitted: bool,
    pub(super) paint_overridden: bool,
    pub(super) selected_property: Option<ResolvedStyleProperty>,
    pub(super) receipt: SequenceLifelineThemeReceipt,
}

impl SequenceLifelineThemeEmission {
    pub(crate) fn from_terminal_writer(
        typed_stroke: Option<&str>,
        typed_stroke_width: Option<f32>,
        typed_stroke_width_won: bool,
        selected_property: Option<ResolvedStyleProperty>,
        paint_overridden: bool,
        receipt: SequenceLifelineThemeReceipt,
    ) -> Self {
        let has_lines = receipt.candidate_count() != 0;
        let complete_line_emission = receipt.has_complete_emission();
        Self {
            paint_emitted: complete_line_emission && typed_stroke.is_some(),
            // A Value winner reaches the terminal CSS block; a Clear winner deliberately keeps
            // the `0.5px` width emitted inline by every actor-line writer. The shared candidate /
            // emission receipt proves both paths without treating Clear as an absent winner.
            stroke_width_emitted: complete_line_emission
                && (typed_stroke_width.is_some() || typed_stroke_width_won),
            paint_overridden: has_lines && paint_overridden,
            selected_property,
            receipt,
        }
    }
}

/// Winner and terminal-emission facts produced by the Sequence Message line writer.
///
/// Message owns one shared Mermaid `signalColor` projection. The receipt retains both fill and
/// stroke winners because an effective stroke takes precedence over fill, while a fill winner is
/// projected into the same signal-color CSS declarations. Candidate and emitted counts stay
/// separate so a compiled route cannot claim evidence unless every concrete terminal line or path
/// reached the writer-owned checkpoint.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceMessageThemeReceipt {
    line: SequenceLineThemeReceipt,
    pub(crate) effect_requested: bool,
    pub(crate) effect_binding_used: bool,
    pub(crate) effect_cleared: bool,
    pub(crate) effect_unhandled: bool,
    emitted_effects: usize,
}

impl SequenceMessageThemeReceipt {
    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        self.line.record_static_style(style);
    }

    pub(crate) fn record_line_candidate(&mut self) {
        self.line.record_line_candidate();
    }

    pub(crate) fn record_line_emission(&mut self) {
        self.line.record_line_emission();
    }

    pub(crate) fn record_effect_emission(&mut self) {
        self.emitted_effects = self.emitted_effects.saturating_add(1);
    }

    pub(super) fn effect_complete(&self) -> bool {
        self.candidate_count() != 0 && self.candidate_count() == self.emitted_effects
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.line.merge(other.line);
        self.effect_requested |= other.effect_requested;
        self.effect_binding_used |= other.effect_binding_used;
        self.effect_cleared |= other.effect_cleared;
        self.effect_unhandled |= other.effect_unhandled;
        self.emitted_effects = self.emitted_effects.max(other.emitted_effects);
    }

    pub(super) fn route_won(
        &self,
        rule_index: usize,
        _selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        self.line.route_won(rule_index, facet)
    }

    pub(super) fn candidate_count(&self) -> usize {
        self.line.candidate_count()
    }

    fn has_complete_emission(&self) -> bool {
        self.line.has_complete_emission()
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct SequenceMessageThemeState {
    pub(super) stroke_emitted: bool,
    pub(super) stroke_width_emitted: bool,
    pub(super) stroke_overridden: bool,
    pub(super) selected_property: Option<ResolvedStyleProperty>,
    pub(super) receipt: SequenceMessageThemeReceipt,
}

impl SequenceMessageThemeState {
    pub(super) fn merge(&mut self, emission: SequenceMessageThemeEmission) {
        self.stroke_emitted |= emission.stroke_emitted;
        self.stroke_width_emitted |= emission.stroke_width_emitted;
        self.stroke_overridden |= emission.stroke_overridden;
        if let Some(selected_property) = emission.selected_property {
            debug_assert!(
                self.selected_property.is_none()
                    || self.selected_property == Some(selected_property),
                "one Sequence artifact must resolve one stable Message paint winner"
            );
            self.selected_property.get_or_insert(selected_property);
        }
        self.receipt.merge(emission.receipt);
    }
}

/// Complete writer-owned emission facts for Sequence Message signal color and width.
#[derive(Debug)]
pub(crate) struct SequenceMessageThemeEmission {
    pub(super) stroke_emitted: bool,
    pub(super) stroke_width_emitted: bool,
    pub(super) stroke_overridden: bool,
    pub(super) selected_property: Option<ResolvedStyleProperty>,
    pub(super) receipt: SequenceMessageThemeReceipt,
}

impl SequenceMessageThemeEmission {
    pub(crate) fn from_terminal_writer(
        typed_stroke: Option<&str>,
        stroke_width_won: bool,
        stroke_overridden: bool,
        selected_property: Option<ResolvedStyleProperty>,
        receipt: SequenceMessageThemeReceipt,
    ) -> Self {
        let has_lines = receipt.candidate_count() != 0;
        let complete_line_emission = receipt.has_complete_emission();
        Self {
            stroke_emitted: complete_line_emission && typed_stroke.is_some(),
            // Clear consumes the winning width by preserving the default message CSS width.
            stroke_width_emitted: complete_line_emission && stroke_width_won,
            stroke_overridden: has_lines && stroke_overridden,
            selected_property,
            receipt,
        }
    }
}

/// Winner, stylesheet, and concrete text facts for Sequence autonumber labels.
///
/// The number marker remains owned by Message stroke. This receipt proves only the independently
/// styled `.sequenceNumber` text and records the exact fill emitted by the final CSS owner.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceNumberLabelThemeReceipt {
    pub(super) static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    pub(super) stylesheet_fill: Option<String>,
    pub(super) typed_stylesheet_emissions: usize,
    pub(super) text_candidates: usize,
    pub(super) emitted_texts: usize,
    pub(super) terminal_fill_mismatch: bool,
}

impl SequenceNumberLabelThemeReceipt {
    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
    }

    pub(crate) fn record_stylesheet_emission(
        &mut self,
        final_fill: &str,
        typed_fill: Option<&str>,
    ) {
        if typed_fill.is_some() {
            self.typed_stylesheet_emissions = self.typed_stylesheet_emissions.saturating_add(1);
        }
        if self
            .stylesheet_fill
            .as_deref()
            .is_some_and(|fill| fill != final_fill)
            || typed_fill.is_some_and(|fill| fill != final_fill)
        {
            self.terminal_fill_mismatch = true;
        }
        self.stylesheet_fill
            .get_or_insert_with(|| final_fill.to_owned());
    }

    pub(crate) fn record_text_candidate(&mut self) {
        self.text_candidates = self.text_candidates.saturating_add(1);
    }

    pub(crate) fn record_text_emission(&mut self, final_fill: Option<&str>) {
        self.emitted_texts = self.emitted_texts.saturating_add(1);
        if self.stylesheet_fill.as_deref() != final_fill {
            self.terminal_fill_mismatch = true;
        }
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.static_winners.extend(other.static_winners);
        self.text_candidates = self.text_candidates.max(other.text_candidates);
        self.emitted_texts = self.emitted_texts.max(other.emitted_texts);
        self.terminal_fill_mismatch |= other.terminal_fill_mismatch;
        self.typed_stylesheet_emissions = self
            .typed_stylesheet_emissions
            .max(other.typed_stylesheet_emissions);
        if let Some(other_fill) = other.stylesheet_fill {
            if self
                .stylesheet_fill
                .as_deref()
                .is_some_and(|fill| fill != other_fill.as_str())
            {
                self.terminal_fill_mismatch = true;
            }
            self.stylesheet_fill.get_or_insert(other_fill);
        }
    }

    pub(super) fn route_applies(
        &self,
        rule_index: usize,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        match selector {
            FamilyThemeSelectorShape::Static { .. } => self
                .static_winners
                .contains(&(rule_index, style_property_for_facet(facet))),
            FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            } => selector.ordinal_domain_intersects_occurrence_count(self.text_candidates),
            FamilyThemeSelectorShape::Ordinal { .. } => false,
        }
    }

    pub(super) fn proves_typed_fill(&self, expected_fill: Option<&str>) -> bool {
        let Some(expected_fill) = expected_fill else {
            return false;
        };
        self.text_candidates != 0
            && self.emitted_texts == self.text_candidates
            && self.typed_stylesheet_emissions == 1
            && self.stylesheet_fill.as_deref() == Some(expected_fill)
            && !self.terminal_fill_mismatch
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct SequenceNumberLabelThemeState {
    pub(super) fill_emitted: bool,
    pub(super) fill_overridden: bool,
    pub(super) receipt: SequenceNumberLabelThemeReceipt,
}

impl SequenceNumberLabelThemeState {
    pub(super) fn merge(&mut self, emission: SequenceNumberLabelThemeEmission) {
        self.fill_emitted |= emission.fill_emitted;
        self.fill_overridden |= emission.fill_overridden;
        self.receipt.merge(emission.receipt);
    }
}

/// Complete terminal facts for the SequenceNumberLabel fill route.
#[derive(Debug)]
pub(crate) struct SequenceNumberLabelThemeEmission {
    pub(super) fill_emitted: bool,
    pub(super) fill_overridden: bool,
    pub(super) receipt: SequenceNumberLabelThemeReceipt,
}

impl SequenceNumberLabelThemeEmission {
    pub(crate) fn from_terminal_writer(
        typed_fill: Option<&str>,
        fill_overridden: bool,
        receipt: SequenceNumberLabelThemeReceipt,
    ) -> Self {
        let has_numbers = receipt.text_candidates != 0;
        Self {
            fill_emitted: receipt.proves_typed_fill(typed_fill),
            fill_overridden: has_numbers && fill_overridden,
            receipt,
        }
    }
}

/// Winner, stylesheet, and concrete `.labelBox` facts for Sequence control surfaces.
///
/// Frame lines and keyword polygons retain separate instances. Stylesheet ownership and
/// actual terminal emission must agree; one surface cannot discharge the other.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceControlThemeReceipt {
    pub(crate) effect_requested: bool,
    pub(crate) effect_binding_used: bool,
    pub(crate) effect_cleared: bool,
    pub(crate) effect_unhandled: Cell<bool>,
    effect_emissions: Cell<usize>,
    pub(super) static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    pub(super) stylesheet_fill: Option<String>,
    pub(super) typed_fill_emissions: usize,
    pub(super) stylesheet_stroke: Option<String>,
    pub(super) typed_stroke_emissions: usize,
    pub(super) stylesheet_mismatch: bool,
    pub(super) surface_candidates: Cell<usize>,
    pub(super) emitted_surfaces: Cell<usize>,
}

impl SequenceControlThemeReceipt {
    pub(crate) fn record_effect_emission(&self) {
        self.effect_emissions
            .set(self.effect_emissions.get().saturating_add(1));
    }

    pub(super) fn effect_complete(&self) -> bool {
        self.effect_requested
            && !self.effect_unhandled.get()
            && self.complete_surfaces()
            && self.effect_emissions.get() == self.surface_candidates.get()
    }

    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
    }

    pub(crate) fn record_stylesheet_emission(
        &mut self,
        final_fill: &str,
        typed_fill: Option<&str>,
        final_stroke: &str,
        typed_stroke: Option<&str>,
    ) {
        if typed_fill.is_some() {
            self.typed_fill_emissions = self.typed_fill_emissions.saturating_add(1);
        }
        if typed_stroke.is_some() {
            self.typed_stroke_emissions = self.typed_stroke_emissions.saturating_add(1);
        }
        if self
            .stylesheet_fill
            .as_deref()
            .is_some_and(|fill| fill != final_fill)
            || typed_fill.is_some_and(|fill| fill != final_fill)
            || self
                .stylesheet_stroke
                .as_deref()
                .is_some_and(|stroke| stroke != final_stroke)
            || typed_stroke.is_some_and(|stroke| stroke != final_stroke)
        {
            self.stylesheet_mismatch = true;
        }
        self.stylesheet_fill
            .get_or_insert_with(|| final_fill.to_owned());
        self.stylesheet_stroke
            .get_or_insert_with(|| final_stroke.to_owned());
    }

    pub(crate) fn record_surface_candidate(&self) {
        self.surface_candidates
            .set(self.surface_candidates.get().saturating_add(1));
    }

    pub(crate) fn record_surface_emission(&self) {
        self.emitted_surfaces
            .set(self.emitted_surfaces.get().saturating_add(1));
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.effect_requested |= other.effect_requested;
        self.effect_binding_used |= other.effect_binding_used;
        self.effect_cleared |= other.effect_cleared;
        self.effect_unhandled
            .set(self.effect_unhandled.get() || other.effect_unhandled.get());
        self.effect_emissions.set(
            self.effect_emissions
                .get()
                .max(other.effect_emissions.get()),
        );
        self.static_winners.extend(other.static_winners);
        self.typed_fill_emissions = self.typed_fill_emissions.max(other.typed_fill_emissions);
        self.typed_stroke_emissions = self
            .typed_stroke_emissions
            .max(other.typed_stroke_emissions);
        self.stylesheet_mismatch |= other.stylesheet_mismatch;
        self.surface_candidates.set(
            self.surface_candidates
                .get()
                .max(other.surface_candidates.get()),
        );
        self.emitted_surfaces.set(
            self.emitted_surfaces
                .get()
                .max(other.emitted_surfaces.get()),
        );
        merge_terminal_value(
            &mut self.stylesheet_fill,
            other.stylesheet_fill,
            &mut self.stylesheet_mismatch,
        );
        merge_terminal_value(
            &mut self.stylesheet_stroke,
            other.stylesheet_stroke,
            &mut self.stylesheet_mismatch,
        );
    }

    pub(super) fn route_won(
        &self,
        rule_index: usize,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        ) && self
            .static_winners
            .contains(&(rule_index, style_property_for_facet(facet)))
    }

    pub(super) fn complete_surfaces(&self) -> bool {
        self.surface_candidates.get() != 0
            && self.surface_candidates.get() == self.emitted_surfaces.get()
    }

    pub(super) fn proves_typed_fill(&self, expected_fill: Option<&str>) -> bool {
        let Some(expected_fill) = expected_fill else {
            return false;
        };
        self.complete_surfaces()
            && self.typed_fill_emissions == 1
            && self.stylesheet_fill.as_deref() == Some(expected_fill)
            && !self.stylesheet_mismatch
    }

    pub(super) fn proves_typed_stroke(&self, expected_stroke: Option<&str>) -> bool {
        let Some(expected_stroke) = expected_stroke else {
            return false;
        };
        self.complete_surfaces()
            && self.typed_stroke_emissions == 1
            && self.stylesheet_stroke.as_deref() == Some(expected_stroke)
            && !self.stylesheet_mismatch
    }
}

fn merge_terminal_value(
    current: &mut Option<String>,
    incoming: Option<String>,
    mismatch: &mut bool,
) {
    let Some(incoming) = incoming else {
        return;
    };
    if current
        .as_deref()
        .is_some_and(|value| value != incoming.as_str())
    {
        *mismatch = true;
    }
    current.get_or_insert(incoming);
}

#[derive(Debug, Clone, Default)]
pub(super) struct SequenceControlThemeState {
    pub(super) fill_emitted: bool,
    pub(super) fill_overridden: bool,
    pub(super) stroke_emitted: bool,
    pub(super) stroke_overridden: bool,
    pub(super) receipt: SequenceControlThemeReceipt,
}

impl SequenceControlThemeState {
    pub(super) fn merge(&mut self, emission: SequenceControlThemeEmission) {
        self.fill_emitted |= emission.fill_emitted;
        self.fill_overridden |= emission.fill_overridden;
        self.stroke_emitted |= emission.stroke_emitted;
        self.stroke_overridden |= emission.stroke_overridden;
        self.receipt.merge(emission.receipt);
    }
}

/// Complete terminal facts for the Sequence Loop fill/stroke tranche.
#[derive(Debug)]
pub(crate) struct SequenceControlThemeEmission {
    pub(super) fill_emitted: bool,
    pub(super) fill_overridden: bool,
    pub(super) stroke_emitted: bool,
    pub(super) stroke_overridden: bool,
    pub(super) receipt: SequenceControlThemeReceipt,
}

impl SequenceControlThemeEmission {
    pub(crate) fn from_terminal_writer(
        typed_fill: Option<&str>,
        fill_overridden: bool,
        typed_stroke: Option<&str>,
        stroke_overridden: bool,
        receipt: SequenceControlThemeReceipt,
    ) -> Self {
        let has_surfaces = receipt.surface_candidates.get() != 0;
        Self {
            fill_emitted: receipt.proves_typed_fill(typed_fill),
            fill_overridden: has_surfaces && fill_overridden,
            stroke_emitted: receipt.proves_typed_stroke(typed_stroke),
            stroke_overridden: has_surfaces && stroke_overridden,
            receipt,
        }
    }
}

/// Winner and terminal-emission facts produced by a Sequence static rectangle writer.
///
/// This receipt is deliberately narrower than the Actor receipt: the current direct tranche owns
/// static Note paint, geometry and effects, and Activation fill and stroke. Each writer records
/// the surfaces it actually emits so evidence cannot be manufactured from the compiled route
/// matrix alone.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceStaticRectThemeReceipt {
    pub(super) static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    pub(super) emitted_rects: usize,
    pub(crate) effect_requested: bool,
    pub(crate) effect_binding_used: bool,
    pub(crate) effect_cleared: bool,
    pub(crate) effect_unhandled: bool,
    emitted_effects: usize,
}

impl SequenceStaticRectThemeReceipt {
    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
    }

    pub(crate) fn record_rect_emission(&mut self) {
        self.emitted_rects = self.emitted_rects.saturating_add(1);
    }

    pub(crate) fn record_effect_emission(&mut self) {
        self.emitted_effects = self.emitted_effects.saturating_add(1);
    }

    pub(super) fn effect_complete(&self, surface_count: usize) -> bool {
        surface_count != 0 && self.emitted_effects == surface_count
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.static_winners.extend(other.static_winners);
        self.emitted_rects = self.emitted_rects.max(other.emitted_rects);
        self.effect_requested |= other.effect_requested;
        self.effect_binding_used |= other.effect_binding_used;
        self.effect_cleared |= other.effect_cleared;
        self.effect_unhandled |= other.effect_unhandled;
        self.emitted_effects = self.emitted_effects.max(other.emitted_effects);
    }

    pub(super) fn route_won(
        &self,
        rule_index: usize,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        ) && self
            .static_winners
            .contains(&(rule_index, style_property_for_facet(facet)))
    }
}

/// Complete terminal emission facts for one Sequence static rectangle paint tranche.
#[derive(Debug, Clone, Default)]
pub(super) struct SequenceStaticRectThemeState {
    pub(super) surface_count: usize,
    pub(super) fill_emitted: bool,
    pub(super) fill_overridden: bool,
    pub(super) stroke_emitted: bool,
    pub(super) stroke_overridden: bool,
    pub(super) receipt: SequenceStaticRectThemeReceipt,
}

impl SequenceStaticRectThemeState {
    pub(super) fn merge(&mut self, emission: SequenceStaticRectThemeEmission) {
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
    pub(super) surface_count: usize,
    pub(super) fill_emitted: bool,
    pub(super) fill_overridden: bool,
    pub(super) stroke_emitted: bool,
    pub(super) stroke_overridden: bool,
    pub(super) receipt: SequenceStaticRectThemeReceipt,
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

#[derive(Debug, Clone, Default)]
struct SequenceTextSurfaceReceipt {
    fill_overridden: bool,
    requires_prepared_math_terminal_evidence: bool,
    required_typography_properties: BTreeSet<ThemeTypographyProperty>,
    expected_font_size_bits: u64,
    stylesheet_fill: Option<String>,
    typed_stylesheet_emissions: usize,
    stylesheet_fill_mismatch: bool,
    label_candidates: Cell<usize>,
    emitted_labels: Cell<usize>,
    emitted_effects: Cell<usize>,
    paint_candidates: Cell<usize>,
    emitted_paint_labels: Cell<usize>,
    unverified_math_typography: RefCell<BTreeSet<ThemeTypographyProperty>>,
    paint_terminal_mismatch: Cell<bool>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SequenceBaseTypographyReceipt {
    expected_font_family: String,
    expected_font_size_bits: u64,
    typed_properties: BTreeSet<ThemeTypographyProperty>,
    terminal_svg_observed: bool,
    stylesheet_verified: bool,
    inherited_font_stack_occurrences: usize,
    inherited_font_size_occurrences: usize,
}

impl SequenceBaseTypographyReceipt {
    fn from_plan(plan: &crate::sequence::SequenceTypographyPlan) -> Self {
        Self {
            expected_font_family: plan.base_font_family_css().to_owned(),
            expected_font_size_bits: plan.base_font_size_px().to_bits(),
            typed_properties: plan.base_typed_properties().clone(),
            terminal_svg_observed: false,
            stylesheet_verified: false,
            inherited_font_stack_occurrences: 0,
            inherited_font_size_occurrences: 0,
        }
    }

    pub(super) const fn terminal_svg_observed(&self) -> bool {
        self.terminal_svg_observed
    }

    pub(super) const fn stylesheet_verified(&self) -> bool {
        self.stylesheet_verified
    }

    pub(super) const fn inherited_occurrences(&self, property: ThemeTypographyProperty) -> usize {
        match property {
            ThemeTypographyProperty::FontStack => self.inherited_font_stack_occurrences,
            ThemeTypographyProperty::FontSize => self.inherited_font_size_occurrences,
            ThemeTypographyProperty::FontWeight
            | ThemeTypographyProperty::FontStyle
            | ThemeTypographyProperty::LineHeight
            | ThemeTypographyProperty::LetterSpacing
            | ThemeTypographyProperty::WordSpacing
            | ThemeTypographyProperty::Transform
            | ThemeTypographyProperty::Decoration
            | ThemeTypographyProperty::TextAlign
            | ThemeTypographyProperty::WhiteSpace
            | ThemeTypographyProperty::Wrap => 0,
        }
    }

    pub(super) fn has_typed_property(&self, property: ThemeTypographyProperty) -> bool {
        self.typed_properties.contains(&property)
    }
}

impl SequenceTextSurfaceReceipt {
    fn record_stylesheet_emission(&mut self, final_fill: &str, typed_fill: Option<&str>) {
        if typed_fill.is_some() {
            self.typed_stylesheet_emissions = self.typed_stylesheet_emissions.saturating_add(1);
        }
        if self
            .stylesheet_fill
            .as_deref()
            .is_some_and(|fill| fill != final_fill)
            || typed_fill.is_some_and(|fill| fill != final_fill)
        {
            self.stylesheet_fill_mismatch = true;
        }
        self.stylesheet_fill
            .get_or_insert_with(|| final_fill.to_owned());
    }

    fn record_candidate(&self) {
        self.label_candidates
            .set(self.label_candidates.get().saturating_add(1));
        self.paint_candidates
            .set(self.paint_candidates.get().saturating_add(1));
    }

    fn record_prepared_math_emission(&self, assurance: &crate::math::PreparedMathStyleAssurance) {
        if !self.requires_prepared_math_terminal_evidence {
            return;
        }
        self.record_candidate();
        self.emitted_labels
            .set(self.emitted_labels.get().saturating_add(1));

        let paint_verified = if self.typed_stylesheet_emissions == 0 {
            true
        } else {
            self.stylesheet_fill
                .as_deref()
                .is_some_and(|fill| assurance.proves_default_foreground(fill))
        };
        if paint_verified {
            self.emitted_paint_labels
                .set(self.emitted_paint_labels.get().saturating_add(1));
        } else {
            self.paint_terminal_mismatch.set(true);
        }

        let mut unverified = self.unverified_math_typography.borrow_mut();
        for property in &self.required_typography_properties {
            let verified = match property {
                ThemeTypographyProperty::FontSize => {
                    assurance.proves_font_size_px(f64::from_bits(self.expected_font_size_bits))
                }
                ThemeTypographyProperty::FontStack
                | ThemeTypographyProperty::FontWeight
                | ThemeTypographyProperty::FontStyle
                | ThemeTypographyProperty::LineHeight
                | ThemeTypographyProperty::LetterSpacing
                | ThemeTypographyProperty::WordSpacing
                | ThemeTypographyProperty::Transform
                | ThemeTypographyProperty::Decoration
                | ThemeTypographyProperty::TextAlign
                | ThemeTypographyProperty::WhiteSpace
                | ThemeTypographyProperty::Wrap => false,
            };
            if !verified {
                unverified.insert(*property);
            }
        }
    }

    fn record_emission(&self) {
        self.emitted_labels
            .set(self.emitted_labels.get().saturating_add(1));
        self.emitted_paint_labels
            .set(self.emitted_paint_labels.get().saturating_add(1));
    }

    fn complete(&self) -> bool {
        self.label_candidates.get() != 0
            && self.label_candidates.get() == self.emitted_labels.get()
            && self.unverified_math_typography.borrow().is_empty()
    }

    fn typography_property_complete(&self, property: ThemeTypographyProperty) -> bool {
        self.required_typography_properties.contains(&property)
            && self.label_candidates.get() != 0
            && self.label_candidates.get() == self.emitted_labels.get()
            && !self.unverified_math_typography.borrow().contains(&property)
    }

    fn proves_typed_fill(&self) -> bool {
        !self.fill_overridden
            && self.paint_candidates.get() != 0
            && self.paint_candidates.get() == self.emitted_paint_labels.get()
            && self.typed_stylesheet_emissions == 1
            && self.stylesheet_fill.is_some()
            && !self.stylesheet_fill_mismatch
            && !self.paint_terminal_mismatch.get()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SequenceRoleFillSummary {
    pub(super) typed_applied: bool,
    pub(super) incomplete: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SequenceRoleTypographyReceipt {
    pub(super) static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    pub(super) config_overrides: BTreeSet<crate::diagram_theme::ThemeTypographyProperty>,
    base_typed_properties: BTreeSet<ThemeTypographyProperty>,
    text_surfaces: [SequenceTextSurfaceReceipt; 7],
    pub(super) effect_requested: bool,
    pub(super) effect_binding_used: bool,
    pub(super) effect_unhandled: bool,
    pub(super) effect_applications: Cell<usize>,
}

impl SequenceRoleTypographyReceipt {
    pub(super) fn from_resolved(
        role: crate::sequence::SequenceTypographyRole,
        typography: &crate::sequence::SequenceResolvedTypography,
    ) -> Self {
        let mut static_winners = BTreeSet::new();
        if let Some(style) = typography.resolved_style() {
            record_style_winners(&mut static_winners, style);
        }
        let mut receipt = Self {
            static_winners,
            config_overrides: typography.config_overrides().clone(),
            base_typed_properties: typography.base_typed_properties().clone(),
            ..Self::default()
        };
        for surface in crate::sequence::SequenceTextSurface::ALL {
            if surface.role() == role {
                let surface_receipt = &mut receipt.text_surfaces[surface.index()];
                surface_receipt.fill_overridden = typography.fill_overridden(surface);
                surface_receipt.requires_prepared_math_terminal_evidence =
                    typography.requires_terminal_evidence_for(surface);
                surface_receipt.required_typography_properties = typography
                    .typed_properties()
                    .union(typography.base_typed_properties())
                    .copied()
                    .collect();
                surface_receipt.expected_font_size_bits =
                    typography.terminal_text_style().font_size.to_bits();
            }
        }
        receipt
    }

    pub(super) fn record_stylesheet_emission(
        &mut self,
        surface: crate::sequence::SequenceTextSurface,
        final_fill: &str,
        typed_fill: Option<&str>,
    ) {
        self.text_surfaces[surface.index()].record_stylesheet_emission(final_fill, typed_fill);
    }

    pub(super) fn record_candidate(&self, surface: crate::sequence::SequenceTextSurface) {
        self.text_surfaces[surface.index()].record_candidate();
    }

    pub(super) fn record_prepared_math_emission(
        &self,
        surface: crate::sequence::SequenceTextSurface,
        assurance: &crate::math::PreparedMathStyleAssurance,
    ) {
        self.text_surfaces[surface.index()].record_prepared_math_emission(assurance);
    }

    pub(super) fn record_emission(&self, surface: crate::sequence::SequenceTextSurface) {
        self.text_surfaces[surface.index()].record_emission();
    }

    pub(super) fn route_won(
        &self,
        rule_index: usize,
        _selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        self.static_winners
            .contains(&(rule_index, style_property_for_facet(facet)))
    }

    pub(super) fn complete(&self) -> bool {
        self.label_candidate_count() != 0
            && self
                .text_surfaces
                .iter()
                .filter(|surface| surface.label_candidates.get() != 0)
                .all(SequenceTextSurfaceReceipt::complete)
    }

    pub(super) fn effect_complete(&self) -> bool {
        self.label_candidate_count() != 0
            && self
                .text_surfaces
                .iter()
                .all(|surface| surface.label_candidates.get() == surface.emitted_effects.get())
    }

    pub(super) fn typography_property_complete(&self, property: ThemeTypographyProperty) -> bool {
        let mut relevant = self.text_surfaces.iter().filter(|surface| {
            surface.required_typography_properties.contains(&property)
                && surface.label_candidates.get() != 0
        });
        let Some(first) = relevant.next() else {
            return false;
        };
        first.typography_property_complete(property)
            && relevant.all(|surface| surface.typography_property_complete(property))
    }

    pub(super) fn base_property_reached_complete_terminal(
        &self,
        property: ThemeTypographyProperty,
    ) -> bool {
        self.base_typed_properties.contains(&property)
            && self.typography_property_complete(property)
    }

    pub(super) fn base_property_terminal_incomplete(
        &self,
        property: ThemeTypographyProperty,
    ) -> bool {
        self.base_typed_properties.contains(&property)
            && self.label_candidate_count() != 0
            && !self.typography_property_complete(property)
    }

    pub(super) fn label_candidate_count(&self) -> usize {
        self.text_surfaces
            .iter()
            .map(|surface| surface.label_candidates.get())
            .sum()
    }

    pub(super) fn fill_summary(&self) -> SequenceRoleFillSummary {
        let mut summary = SequenceRoleFillSummary::default();
        for surface in &self.text_surfaces {
            if surface.label_candidates.get() == 0 || surface.fill_overridden {
                continue;
            }
            if surface.proves_typed_fill() {
                summary.typed_applied = true;
            } else {
                summary.incomplete = true;
            }
        }
        summary
    }

    #[cfg(test)]
    pub(super) fn seed_complete_surface(
        &mut self,
        surface: crate::sequence::SequenceTextSurface,
        final_fill: &str,
        typed_fill: Option<&str>,
    ) {
        self.record_stylesheet_emission(surface, final_fill, typed_fill);
        self.record_candidate(surface);
        self.record_emission(surface);
    }
}

/// One operation-local receipt shared by every Sequence role-label terminal writer.
#[derive(Debug, Clone)]
pub(crate) struct SequenceTypographyThemeReceipt {
    pub(super) base: SequenceBaseTypographyReceipt,
    pub(super) actor: SequenceRoleTypographyReceipt,
    pub(super) message: SequenceRoleTypographyReceipt,
    pub(super) note: SequenceRoleTypographyReceipt,
    pub(super) loop_label: SequenceRoleTypographyReceipt,
}

impl SequenceTypographyThemeReceipt {
    pub(crate) fn from_plan(plan: &crate::sequence::SequenceTypographyPlan) -> Self {
        Self {
            base: SequenceBaseTypographyReceipt::from_plan(plan),
            actor: SequenceRoleTypographyReceipt::from_resolved(
                crate::sequence::SequenceTypographyRole::Actor,
                plan.actor(),
            ),
            message: SequenceRoleTypographyReceipt::from_resolved(
                crate::sequence::SequenceTypographyRole::Message,
                plan.message(),
            ),
            note: SequenceRoleTypographyReceipt::from_resolved(
                crate::sequence::SequenceTypographyRole::Note,
                plan.note(),
            ),
            loop_label: SequenceRoleTypographyReceipt::from_resolved(
                crate::sequence::SequenceTypographyRole::Loop,
                plan.loop_label(),
            ),
        }
    }

    fn requires_terminal_svg_observation(&self) -> bool {
        !self.base.typed_properties.is_empty()
    }

    pub(crate) fn record_terminal_svg(
        &mut self,
        svg: &str,
        diagram_id: &str,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<(), crate::resources::OperationWorkError> {
        if !self.requires_terminal_svg_observation() {
            return Ok(());
        }
        work_meter.checkpoint(merman_core::OperationPhase::Emit)?;
        work_meter.charge(svg.len().div_ceil(64).max(1))?;
        let Ok(document) = roxmltree::Document::parse(svg) else {
            return Ok(());
        };
        let root = document.root_element();
        if !root.has_tag_name(("http://www.w3.org/2000/svg", "svg"))
            || root.attribute("id") != Some(diagram_id)
        {
            return Ok(());
        }
        let mut styles = document
            .descendants()
            .filter(|node| node.has_tag_name(("http://www.w3.org/2000/svg", "style")));
        let stylesheet = styles
            .next()
            .and_then(|style| style.text())
            .unwrap_or_default();
        let id = crate::svg::escape_css_identifier(diagram_id);
        let unique_stylesheet = styles.next().is_none();
        self.base.terminal_svg_observed = unique_stylesheet;
        self.base.stylesheet_verified = unique_stylesheet
            && typography_rule_matches(
                stylesheet,
                &format!("#{id}"),
                &self.base.expected_font_family,
                self.base.expected_font_size_bits,
            )
            && typography_rule_matches(
                stylesheet,
                &format!("#{id} svg"),
                &self.base.expected_font_family,
                self.base.expected_font_size_bits,
            )
            && font_family_variable_rule_matches(
                stylesheet,
                &format!("#{id} :root"),
                &self.base.expected_font_family,
            );
        for text in document.descendants().filter(is_base_inherited_text) {
            if terminal_text_inherits_property(text, ThemeTypographyProperty::FontStack) {
                self.base.inherited_font_stack_occurrences =
                    self.base.inherited_font_stack_occurrences.saturating_add(1);
            }
            if terminal_text_inherits_property(text, ThemeTypographyProperty::FontSize) {
                self.base.inherited_font_size_occurrences =
                    self.base.inherited_font_size_occurrences.saturating_add(1);
            }
        }
        Ok(())
    }

    pub(super) fn base_property_applied(&self, property: ThemeTypographyProperty) -> bool {
        if !self.base.has_typed_property(property) {
            return false;
        }
        let roles = [&self.actor, &self.message, &self.note, &self.loop_label];
        if roles
            .iter()
            .any(|role| role.base_property_reached_complete_terminal(property))
        {
            return true;
        }
        self.base.inherited_occurrences(property) != 0
    }

    pub(super) fn base_property_incomplete(&self, property: ThemeTypographyProperty) -> bool {
        [&self.actor, &self.message, &self.note, &self.loop_label]
            .iter()
            .any(|role| role.base_property_terminal_incomplete(property))
    }

    /// General Text fill is a fallback beneath the role-specific targets. Suppress it only
    /// after every actual role surface proves its own winning fill reached the terminal output.
    pub(super) fn role_fills_cover_all_labels(&self) -> bool {
        self.role_label_candidate_count() != 0
            && [&self.actor, &self.message, &self.note, &self.loop_label]
                .iter()
                .all(|role| {
                    role.text_surfaces
                        .iter()
                        .filter(|surface| surface.label_candidates.get() != 0)
                        .all(|surface| surface.complete() && surface.proves_typed_fill())
                })
    }

    pub(super) fn role_label_candidate_count(&self) -> usize {
        [&self.actor, &self.message, &self.note, &self.loop_label]
            .iter()
            .map(|role| role.label_candidate_count())
            .sum()
    }

    pub(super) fn base_relevant_occurrences(&self) -> usize {
        self.role_label_candidate_count().saturating_add(
            self.base
                .inherited_font_stack_occurrences
                .max(self.base.inherited_font_size_occurrences),
        )
    }

    pub(super) fn base_property_relevant_occurrences(
        &self,
        property: ThemeTypographyProperty,
    ) -> usize {
        let role_candidates = [&self.actor, &self.message, &self.note, &self.loop_label]
            .iter()
            .filter(|role| role.base_typed_properties.contains(&property))
            .map(|role| role.label_candidate_count())
            .sum::<usize>();
        role_candidates.saturating_add(self.base.inherited_occurrences(property))
    }

    pub(super) fn role(
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

    pub(crate) fn configure_text_effect(
        &mut self,
        role: crate::sequence::SequenceTypographyRole,
        binding: bool,
        unhandled: bool,
    ) {
        let receipt = match role {
            crate::sequence::SequenceTypographyRole::Actor => &mut self.actor,
            crate::sequence::SequenceTypographyRole::Message => &mut self.message,
            crate::sequence::SequenceTypographyRole::Note => &mut self.note,
            crate::sequence::SequenceTypographyRole::Loop => &mut self.loop_label,
        };
        receipt.effect_requested = true;
        receipt.effect_binding_used = binding;
        receipt.effect_unhandled = unhandled;
        // Math terminals still count when only an effect was requested. A prepared
        // foreground/font assurance does not establish that a filter was consumed.
        for surface in crate::sequence::SequenceTextSurface::ALL {
            if surface.role() == role {
                receipt.text_surfaces[surface.index()].requires_prepared_math_terminal_evidence =
                    true;
            }
        }
    }

    pub(crate) fn record_missing_text_effect(&self, surface: crate::sequence::SequenceTextSurface) {
        if self.role(surface.role()).effect_requested {
            self.record_candidate(surface);
        }
    }

    pub(crate) fn record_text_effect(
        &self,
        surface: crate::sequence::SequenceTextSurface,
        filtered: bool,
    ) {
        let count = &self.role(surface.role()).text_surfaces[surface.index()].emitted_effects;
        count.set(count.get().saturating_add(1));
        if filtered {
            let applications = &self.role(surface.role()).effect_applications;
            applications.set(applications.get().saturating_add(1));
        }
    }

    pub(crate) fn record_candidate(&self, surface: crate::sequence::SequenceTextSurface) {
        self.role(surface.role()).record_candidate(surface);
    }

    pub(crate) fn record_prepared_math_emission(
        &self,
        surface: crate::sequence::SequenceTextSurface,
        assurance: &crate::math::PreparedMathStyleAssurance,
    ) {
        self.role(surface.role())
            .record_prepared_math_emission(surface, assurance);
    }

    pub(crate) fn record_emission(&self, surface: crate::sequence::SequenceTextSurface) {
        self.role(surface.role()).record_emission(surface);
    }

    pub(crate) fn record_terminal_text(&self, surface: crate::sequence::SequenceTextSurface) {
        self.record_candidate(surface);
        self.record_emission(surface);
    }

    pub(crate) fn record_stylesheet_emission(
        &mut self,
        surface: crate::sequence::SequenceTextSurface,
        final_fill: &str,
        typed_fill: Option<&str>,
    ) {
        match surface.role() {
            crate::sequence::SequenceTypographyRole::Actor => &mut self.actor,
            crate::sequence::SequenceTypographyRole::Message => &mut self.message,
            crate::sequence::SequenceTypographyRole::Note => &mut self.note,
            crate::sequence::SequenceTypographyRole::Loop => &mut self.loop_label,
        }
        .record_stylesheet_emission(surface, final_fill, typed_fill);
    }
}

fn typography_rule_matches(
    stylesheet: &str,
    selector: &str,
    expected_font_family: &str,
    expected_font_size_bits: u64,
) -> bool {
    let Some(body) = unique_rule_body(stylesheet, selector) else {
        return false;
    };
    let mut font_family_count = 0usize;
    let mut font_size_count = 0usize;
    let valid = visit_valid_style_declarations(body, |declaration| match declaration.property() {
        "font-family" => {
            font_family_count = font_family_count.saturating_add(1);
            declaration.value() == expected_font_family
        }
        "font-size" => {
            font_size_count = font_size_count.saturating_add(1);
            declaration
                .value()
                .strip_suffix("px")
                .and_then(|value| value.parse::<f64>().ok())
                .is_some_and(|value| value.to_bits() == expected_font_size_bits)
        }
        _ => true,
    });
    valid && font_family_count == 1 && font_size_count == 1
}

fn font_family_variable_rule_matches(
    stylesheet: &str,
    selector: &str,
    expected_font_family: &str,
) -> bool {
    let Some(body) = unique_rule_body(stylesheet, selector) else {
        return false;
    };
    let mut declaration_count = 0usize;
    let valid = visit_valid_style_declarations(body, |declaration| {
        if declaration.property() == "--mermaid-font-family" {
            declaration_count = declaration_count.saturating_add(1);
            declaration.value() == expected_font_family
        } else {
            true
        }
    });
    valid && declaration_count == 1
}

fn visit_valid_style_declarations(
    declaration_list: &str,
    mut visit: impl FnMut(&crate::mermaid_style::ParsedStyleDeclaration<'_>) -> bool,
) -> bool {
    let mut valid = true;
    let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
    let _ = crate::mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
        declaration_list,
        &mut checkpoint,
        |boundary| {
            let Some(declaration) = crate::mermaid_style::parse_style_declaration(boundary.raw())
            else {
                valid = false;
                return Ok(false);
            };
            if visit(&declaration) {
                Ok(true)
            } else {
                valid = false;
                Ok(false)
            }
        },
    );
    valid
}

fn is_base_inherited_text(node: &roxmltree::Node<'_, '_>) -> bool {
    node.has_tag_name(("http://www.w3.org/2000/svg", "text"))
        && node
            .attribute("class")
            .is_none_or(|classes| classes.trim().is_empty())
        && !node
            .ancestors()
            .skip(1)
            .any(|ancestor| ancestor.has_tag_name(("http://www.w3.org/2000/svg", "foreignObject")))
        && node
            .descendants()
            .filter_map(|descendant| descendant.text())
            .any(|text| !text.trim().is_empty())
}

fn terminal_text_inherits_property(
    text: roxmltree::Node<'_, '_>,
    property: ThemeTypographyProperty,
) -> bool {
    let property_name = match property {
        ThemeTypographyProperty::FontStack => "font-family",
        ThemeTypographyProperty::FontSize => "font-size",
        ThemeTypographyProperty::FontWeight
        | ThemeTypographyProperty::FontStyle
        | ThemeTypographyProperty::LineHeight
        | ThemeTypographyProperty::LetterSpacing
        | ThemeTypographyProperty::WordSpacing
        | ThemeTypographyProperty::Transform
        | ThemeTypographyProperty::Decoration
        | ThemeTypographyProperty::TextAlign
        | ThemeTypographyProperty::WhiteSpace
        | ThemeTypographyProperty::Wrap => return false,
    };
    if text.attribute(property_name).is_some() {
        return false;
    }
    let mut has_override = false;
    let valid = visit_valid_style_declarations(
        text.attribute("style").unwrap_or_default(),
        |declaration| {
            has_override |=
                declaration.property() == property_name || declaration.property() == "font";
            true
        },
    );
    valid && !has_override
}

fn unique_rule_body<'a>(stylesheet: &'a str, selector: &str) -> Option<&'a str> {
    let prefix = format!("{selector}{{");
    let mut matches = stylesheet.match_indices(&prefix);
    let (offset, _) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let body = &stylesheet[offset.saturating_add(prefix.len())..];
    Some(&body[..body.find('}')?])
}

pub(super) fn record_style_winners(
    winners: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
    style: &ResolvedThemeStyle,
) {
    winners.extend(
        style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn math_receipt(
        properties: impl IntoIterator<Item = ThemeTypographyProperty>,
        typed_fill: Option<&str>,
    ) -> SequenceTypographyThemeReceipt {
        let mut receipt = SequenceTypographyThemeReceipt {
            base: SequenceBaseTypographyReceipt::default(),
            actor: SequenceRoleTypographyReceipt::default(),
            message: SequenceRoleTypographyReceipt::default(),
            note: SequenceRoleTypographyReceipt::default(),
            loop_label: SequenceRoleTypographyReceipt::default(),
        };
        let surface = &mut receipt.message.text_surfaces
            [crate::sequence::SequenceTextSurface::MessageLabel.index()];
        surface.requires_prepared_math_terminal_evidence = true;
        surface.required_typography_properties = properties.into_iter().collect();
        surface.expected_font_size_bits = 18.0_f64.to_bits();
        if let Some(fill) = typed_fill {
            surface.record_stylesheet_emission(fill, Some(fill));
        }
        receipt
    }

    #[test]
    fn text_effect_requires_every_candidate_and_the_correct_surface() {
        let mut receipt = math_receipt([], None);
        let note = crate::sequence::SequenceTextSurface::NoteLabel;
        receipt.configure_text_effect(crate::sequence::SequenceTypographyRole::Note, true, false);
        receipt.record_terminal_text(note);
        receipt.record_terminal_text(note);
        receipt.record_text_effect(crate::sequence::SequenceTextSurface::MessageLabel, true);
        assert!(!receipt.note.effect_complete());
        receipt.record_text_effect(note, true);
        assert!(!receipt.note.effect_complete());
        receipt.record_text_effect(note, true);
        assert!(receipt.note.effect_complete());
        receipt.record_missing_text_effect(note);
        assert!(!receipt.note.effect_complete());
    }

    #[test]
    fn prepared_math_fill_rejects_a_backend_foreground_that_disagrees_with_the_typed_winner() {
        let surface = crate::sequence::SequenceTextSurface::MessageLabel;
        let receipt = math_receipt([], Some("#e0f2fe"));
        receipt.record_prepared_math_emission(
            surface,
            &crate::math::PreparedMathStyleAssurance::compiled_ratex("#000000", 18.0),
        );

        let fill = receipt.message.fill_summary();
        assert!(!fill.typed_applied);
        assert!(fill.incomplete);
    }

    #[test]
    fn compiled_prepared_math_proves_matching_fill_and_font_size() {
        let surface = crate::sequence::SequenceTextSurface::MessageLabel;
        let receipt = math_receipt([ThemeTypographyProperty::FontSize], Some("#e0f2fe"));
        receipt.record_prepared_math_emission(
            surface,
            &crate::math::PreparedMathStyleAssurance::compiled_ratex("#e0f2fe", 18.0),
        );

        let fill = receipt.message.fill_summary();
        assert!(fill.typed_applied);
        assert!(!fill.incomplete);
        assert!(
            receipt
                .message
                .typography_property_complete(ThemeTypographyProperty::FontSize)
        );
    }

    #[test]
    fn prepared_math_does_not_claim_a_font_stack_that_the_backend_did_not_use() {
        let surface = crate::sequence::SequenceTextSurface::MessageLabel;
        let receipt = math_receipt([ThemeTypographyProperty::FontStack], None);
        receipt.record_prepared_math_emission(
            surface,
            &crate::math::PreparedMathStyleAssurance::compiled_ratex("#e0f2fe", 18.0),
        );

        assert!(
            !receipt
                .message
                .typography_property_complete(ThemeTypographyProperty::FontStack)
        );
        assert!(!receipt.message.complete());
    }

    #[test]
    fn opaque_html_math_backend_cannot_claim_typed_terminal_style() {
        let surface = crate::sequence::SequenceTextSurface::MessageLabel;
        let receipt = math_receipt([ThemeTypographyProperty::FontSize], Some("#e0f2fe"));
        receipt.record_prepared_math_emission(
            surface,
            &crate::math::PreparedMathStyleAssurance::opaque_html("#e0f2fe", 18.0),
        );

        assert!(receipt.message.fill_summary().incomplete);
        assert!(
            !receipt
                .message
                .typography_property_complete(ThemeTypographyProperty::FontSize)
        );
    }

    #[test]
    fn terminal_svg_observation_skips_work_without_typed_base_typography() {
        let mut receipt = math_receipt([], None);
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        receipt
            .record_terminal_svg("not even xml", "sequence-test", &meter)
            .expect("an irrelevant terminal SVG requires no observation");

        assert_eq!(meter.used(), 0);
        assert!(!receipt.base.terminal_svg_observed());
    }

    #[test]
    fn terminal_svg_observation_is_charged_before_parsing() {
        let mut receipt = math_receipt([], None);
        receipt
            .base
            .typed_properties
            .insert(ThemeTypographyProperty::FontStack);
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let invalid_svg = "x".repeat(129);

        receipt
            .record_terminal_svg(&invalid_svg, "sequence-test", &meter)
            .expect("an unbounded meter admits the observation");

        assert_eq!(meter.used(), 3);
        assert!(!receipt.base.terminal_svg_observed());
    }

    #[test]
    fn inline_font_family_with_quoted_semicolon_is_not_counted_as_inherited() {
        let document = roxmltree::Document::parse(
            r#"<svg><text style='font-family:"a;b",sans-serif'>Visible</text></svg>"#,
        )
        .expect("valid sequence text fixture");
        let text = document.root_element().first_element_child().expect("text");

        assert!(!terminal_text_inherits_property(
            text,
            ThemeTypographyProperty::FontStack
        ));
    }

    #[test]
    fn malformed_inline_typography_is_fail_closed_for_inherited_evidence() {
        let document = roxmltree::Document::parse(
            r#"<svg><text style='font-family:"unterminated'>Visible</text></svg>"#,
        )
        .expect("valid sequence text fixture");
        let text = document.root_element().first_element_child().expect("text");

        assert!(!terminal_text_inherits_property(
            text,
            ThemeTypographyProperty::FontStack
        ));
    }
}
