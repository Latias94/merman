use crate::Result;
use crate::model::{
    LayoutCluster, LayoutNode, SequenceDiagramLayout, SequenceParticipantBoxLayout,
    SequenceTextLayout,
};
use crate::resources::OperationWorkMeter;
#[cfg(all(test, feature = "diagram-sequence"))]
use crate::resources::RenderResourcePolicy;
use crate::text::TextMeasurer;
use merman_model::OperationPhase;
use merman_model::families::sequence::{
    SequenceControlKind, SequenceDiagramModel, SequenceMessage,
};
use merman_model::presentation::PresentationConfig;
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::Arc;

const SEQUENCE_ACTOR_LAYOUT_WORK_UNITS: usize = 12;
const SEQUENCE_MESSAGE_LAYOUT_WORK_UNITS: usize = 12;
const SEQUENCE_BOX_LAYOUT_WORK_UNITS: usize = 4;
const SEQUENCE_BOX_MEMBERSHIP_WORK_UNITS: usize = 2;
const SEQUENCE_POPUP_LINK_LAYOUT_WORK_UNITS: usize = 2;
const SEQUENCE_TEXT_LINEAR_REPLAY_WORK_UNITS_PER_BYTE: usize = 8;
const SEQUENCE_CHECKPOINT_INTERVAL: usize = 64;

mod activation;
mod activation_geometry;
mod actors;
mod block_collection;
mod block_geometry;
mod block_steps;
pub(crate) mod config;
mod constants;
mod drawing_geometry;
mod math_artifact;
mod message_metrics;
mod messages;
mod metrics;
mod note_artifact;
mod notes;
mod orchestration;
mod rect;
mod root_bounds;
mod terminal_css;
mod terminal_theme;
mod text_effect;
mod theme_binding;
mod theme_evidence;
mod typography;
pub(crate) use terminal_css::SequencePreparedCss;
pub(crate) use terminal_theme::{
    SequencePreparedNumberTheme, SequencePreparedStaticRectTheme, SequencePreparedTerminalTheme,
};
pub(crate) use text_effect::SequencePreparedTextEffect;
pub(crate) use theme_binding::SequenceCompatBinding;

pub(crate) use activation::{sequence_activation_stack_bounds, sequence_activation_start_x};
pub(crate) use activation_geometry::SequencePreparedActivationGeometry;
pub(crate) use block_steps::SequenceBlockLabelBoxMetrics;
pub(crate) use constants::{
    SEQUENCE_FRAME_SIDE_PAD_PX, SEQUENCE_GLYPH_BAND_HEIGHT, SEQUENCE_MESSAGE_WRAP_PADDING_SIDES,
    SequenceActorBands, SequenceActorLabelFootprint,
};
pub(crate) use drawing_geometry::SequenceFramePlacement;
pub(crate) use math_artifact::{
    SequenceMathArtifactStore, SequenceMathOccurrence, SequenceMathSidecar,
    SequenceMathSidecarBuilder,
};
#[cfg(test)]
pub(crate) use metrics::measure_prepared_sequence_math_label;
pub(crate) use metrics::{
    SequenceDrawnTextNode, SequenceMathHeightMode, measure_sequence_drawn_line_height,
    prepare_sequence_text_line, prepared_sequence_math_terminal_geometry,
    sequence_drawn_text_first_y, sequence_drawn_text_style, sequence_drawn_text_y,
    sequence_inline_font_family, wrap_sequence_label_like_mermaid_lines,
};
pub(crate) use note_artifact::{
    SequenceNoteWrapWidths, SequencePreparedNoteLabelView, SequencePreparedNoteLabels,
};
pub(crate) use notes::{
    SequenceWrappedSideNoteLabelContext, prepare_wrapped_side_note_label,
    sequence_note_final_wrapped_lines, typed_wrapped_side_note_style,
};

use actors::{SequenceActorLayoutPlan, SequenceActorLayoutPlanContext, plan_sequence_actors};
use block_steps::{BlockStepPlanContext, calculate_sequence_block_widths};
use config::SequenceLayoutSettings;
use drawing_geometry::{SequenceFrameGeometry, prepare_sequence_frame_geometry};
use message_metrics::SequenceMessageMetricSidecar;
use orchestration::{SequenceLayoutGraph, SequenceLayoutGraphContext, build_sequence_layout_graph};
use rect::{SequenceRectStackBoundsContext, sequence_rect_stack_x_bounds};
use root_bounds::{
    SequenceRootBoundsContext, SequenceRootGeometry, prepare_sequence_root_geometry,
};
pub(crate) use theme_evidence::{
    SequenceActorThemeReceipt, SequenceControlThemeEmission, SequenceControlThemeReceipt,
    SequenceLifelineThemeEmission, SequenceLifelineThemeReceipt, SequenceMessageThemeEmission,
    SequenceMessageThemeReceipt, SequenceNumberLabelThemeEmission, SequenceNumberLabelThemeReceipt,
    SequenceStaticRectThemeEmission, SequenceStaticRectThemeReceipt, SequenceThemeEvidenceRecorder,
    SequenceTypographyThemeReceipt,
};
pub(crate) use typography::{
    SequenceResolvedTypography, SequenceTerminalForegroundProvenance, SequenceTerminalTextStyle,
    SequenceTextSurface, SequenceTypographyPlan, SequenceTypographyRole,
};

/// Phase-aware cancellation projection for Sequence text and math callbacks.
///
/// Text measurement traits remain infallible for host compatibility. Sequence-owned orchestration
/// therefore checks the operation immediately before and after every opaque callback and inside
/// each label-local loop, returning cancellation through the surrounding fallible render path.
#[derive(Clone, Copy)]
pub(crate) struct SequenceTextCheckpoints<'a> {
    work_meter: &'a OperationWorkMeter,
    phase: OperationPhase,
}

impl<'a> SequenceTextCheckpoints<'a> {
    pub(crate) const fn for_phase(
        work_meter: &'a OperationWorkMeter,
        phase: OperationPhase,
    ) -> Self {
        Self { work_meter, phase }
    }

    pub(crate) fn checkpoint(self) -> Result<()> {
        self.work_meter.checkpoint(self.phase).map_err(Into::into)
    }
}

/// Non-billing cancellation projection shared by Sequence-owned passes.
///
/// The phase is explicit so layout and SVG emission use one cancellation protocol without making
/// domain geometry depend on an SVG-private helper.
#[derive(Clone, Copy)]
pub(crate) struct SequenceOperationCheckpoints<'a> {
    work_meter: &'a OperationWorkMeter,
    phase: OperationPhase,
}

impl<'a> SequenceOperationCheckpoints<'a> {
    const fn new(work_meter: &'a OperationWorkMeter) -> Self {
        Self::for_layout(work_meter)
    }

    pub(crate) const fn for_layout(work_meter: &'a OperationWorkMeter) -> Self {
        Self::for_phase(work_meter, OperationPhase::Layout)
    }

    pub(crate) const fn for_emit(work_meter: &'a OperationWorkMeter) -> Self {
        Self::for_phase(work_meter, OperationPhase::Emit)
    }

    pub(crate) const fn for_phase(
        work_meter: &'a OperationWorkMeter,
        phase: OperationPhase,
    ) -> Self {
        Self { work_meter, phase }
    }

    pub(crate) fn checkpoint(self) -> Result<()> {
        self.work_meter.checkpoint(self.phase).map_err(Into::into)
    }

    pub(crate) fn checkpoint_loop(self, iteration: usize) -> Result<()> {
        if iteration.is_multiple_of(SEQUENCE_CHECKPOINT_INTERVAL) {
            self.checkpoint()?;
        }
        Ok(())
    }

    pub(crate) const fn text(self) -> SequenceTextCheckpoints<'a> {
        SequenceTextCheckpoints::for_phase(self.work_meter, self.phase)
    }
}

type SequenceLayoutCheckpoints<'a> = SequenceOperationCheckpoints<'a>;

/// Source-derived box title and spacing retained from actor layout.
#[derive(Debug)]
pub(crate) struct SequenceBoxLayout {
    pub(crate) label: Option<String>,
    pub(crate) margin: f64,
    pub(crate) x: Option<f64>,
    pub(crate) width: f64,
}

/// Private Sequence render artifact that keeps operation-owned measurements attached to layout.
///
/// Do not expose or detach this from the paired semantic model: the metric sidecar is valid only
/// for that immutable model, text style, and built-in measurement route.
#[derive(Debug)]
pub(crate) struct SequencePreparedArtifact {
    layout: SequenceDiagramLayout,
    visible_model: Option<Box<SequenceDiagramModel>>,
    actor_popup_layouts: HashMap<String, actors::SequenceActorPopupLayout>,
    actor_label_footprints: Vec<Option<SequenceActorLabelFootprint>>,
    wrapped_note_text: SequencePreparedNoteLabels,
    message_metrics: SequenceMessageMetricSidecar,
    box_indices: Vec<Option<usize>>,
    frame_placements: Vec<Option<SequenceFramePlacement>>,
    math_sidecar: SequenceMathSidecar,
    theme_evidence: SequenceThemeEvidenceRecorder,
    effect_evidence: crate::diagram_theme::SvgShadowEvidenceRecorder,
    expected_effect_applications: std::cell::Cell<usize>,
    typography: Arc<SequenceTypographyPlan>,
    terminal_theme: SequencePreparedTerminalTheme,
    activation_geometry: SequencePreparedActivationGeometry,
    activation_theme: SequencePreparedStaticRectTheme,
    css: SequencePreparedCss,
    block_label_box_metrics: SequenceBlockLabelBoxMetrics,
}

impl SequencePreparedArtifact {
    pub(crate) fn model_for_render<'a>(
        &'a self,
        source: &'a SequenceDiagramModel,
    ) -> &'a SequenceDiagramModel {
        self.visible_model.as_deref().unwrap_or(source)
    }

    pub(crate) fn effect_evidence(&self) -> &crate::diagram_theme::SvgShadowEvidenceRecorder {
        &self.effect_evidence
    }

    pub(crate) fn expected_effect_applications(&self) -> &std::cell::Cell<usize> {
        &self.expected_effect_applications
    }

    pub(crate) fn box_indices(&self) -> &[Option<usize>] {
        &self.box_indices
    }

    pub(crate) fn frame_placements(&self) -> &[Option<SequenceFramePlacement>] {
        &self.frame_placements
    }

    pub(crate) fn layout(&self) -> &SequenceDiagramLayout {
        &self.layout
    }

    pub(crate) const fn theme_evidence(&self) -> &SequenceThemeEvidenceRecorder {
        &self.theme_evidence
    }

    pub(crate) const fn math_sidecar(&self) -> &SequenceMathSidecar {
        &self.math_sidecar
    }

    pub(crate) fn wrapped_note_text(&self) -> &SequencePreparedNoteLabels {
        &self.wrapped_note_text
    }

    pub(crate) fn typography(&self) -> &SequenceTypographyPlan {
        self.typography.as_ref()
    }

    pub(crate) const fn number_theme(&self) -> &SequencePreparedNumberTheme {
        &self.terminal_theme.number
    }

    pub(crate) const fn terminal_theme(&self) -> &SequencePreparedTerminalTheme {
        &self.terminal_theme
    }

    pub(crate) const fn activation_geometry(&self) -> &SequencePreparedActivationGeometry {
        &self.activation_geometry
    }

    pub(crate) const fn activation_theme(&self) -> &SequencePreparedStaticRectTheme {
        &self.activation_theme
    }

    pub(crate) const fn css(&self) -> &SequencePreparedCss {
        &self.css
    }

    pub(crate) const fn block_label_box_metrics(&self) -> SequenceBlockLabelBoxMetrics {
        self.block_label_box_metrics
    }

    pub(crate) const fn actor_popup_layouts(
        &self,
    ) -> &HashMap<String, actors::SequenceActorPopupLayout> {
        &self.actor_popup_layouts
    }

    pub(crate) fn actor_label_footprints(&self) -> &[Option<SequenceActorLabelFootprint>] {
        &self.actor_label_footprints
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceLayoutWorkShape {
    actors: usize,
    messages: usize,
    boxes: usize,
    box_memberships: usize,
    popup_links: usize,
    text_work: usize,
}

impl SequenceLayoutWorkShape {
    fn work_units(self) -> Option<usize> {
        // Sequence layout has a fixed number of actor and message passes: measurement, spacing,
        // geometry construction, frame propagation, final drawing DTOs and final bounds. The
        // message term also admits activation compaction and frame source/emission indexing;
        // each retained rectangle and index is bounded by an admitted semantic occurrence.
        // Box membership is scanned
        // twice by Mermaid-compatible actor margin and actor-to-box assignment. Popup labels are
        // measured once with ActorLabel typography. Nested frame accumulators are propagated to
        // their parent once, so block depth does not multiply the per-message term.
        self.actors
            .checked_mul(SEQUENCE_ACTOR_LAYOUT_WORK_UNITS)?
            .checked_add(
                self.messages
                    .checked_mul(SEQUENCE_MESSAGE_LAYOUT_WORK_UNITS)?,
            )?
            .checked_add(self.boxes.checked_mul(SEQUENCE_BOX_LAYOUT_WORK_UNITS)?)?
            .checked_add(
                self.box_memberships
                    .checked_mul(SEQUENCE_BOX_MEMBERSHIP_WORK_UNITS)?,
            )?
            .checked_add(
                self.popup_links
                    .checked_mul(SEQUENCE_POPUP_LINK_LAYOUT_WORK_UNITS)?,
            )
            .and_then(|work| work.checked_add(self.text_work))
    }
}

fn sequence_label_text_work_units(byte_len: usize, wraps: bool) -> Option<usize> {
    // `str::len()` gives an O(1), conservative upper bound for scalar/word scans. The linear
    // term covers the fixed layout, bounds, and SVG replay envelope. Wrapped labels additionally
    // pay a squared-byte envelope before any measurement callback because Mermaid-compatible
    // wrapping repeatedly materializes and measures growing word/line prefixes.
    let linear = byte_len.checked_mul(SEQUENCE_TEXT_LINEAR_REPLAY_WORK_UNITS_PER_BYTE)?;
    let wrapping = if wraps {
        byte_len.checked_mul(byte_len)?
    } else {
        0
    };
    linear.checked_add(wrapping)
}

fn sequence_message_text_work_units(message: &SequenceMessage) -> Option<usize> {
    if let Some(semantics) = message.control_semantics() {
        if semantics.kind == SequenceControlKind::Rect {
            return sequence_label_text_work_units(message.message_text().len(), false);
        }
        if !semantics.consumes_text() {
            return Some(0);
        }
        let byte_len = message.message_text().len().checked_add(2)?;
        return sequence_label_text_work_units(byte_len, true);
    }
    sequence_label_text_work_units(message.message_text().len(), message.wrap)
}

fn sequence_text_work_units_controlled(
    model: &SequenceDiagramModel,
    diagram_title: Option<&str>,
    checkpoints: SequenceLayoutCheckpoints<'_>,
) -> Result<Option<usize>> {
    let mut work_units = 0usize;
    let mut label_index = 0usize;
    let mut include = |label_work: Option<usize>| -> Result<bool> {
        checkpoints.checkpoint_loop(label_index)?;
        label_index = label_index.saturating_add(1);
        let Some(label_work) = label_work else {
            return Ok(false);
        };
        let Some(next) = work_units.checked_add(label_work) else {
            return Ok(false);
        };
        work_units = next;
        Ok(true)
    };

    for actor_id in &model.actor_order {
        if let Some(actor) = model.actors.get(actor_id) {
            if !include(sequence_label_text_work_units(
                actor.description.len(),
                actor.wrap,
            ))? {
                return Ok(None);
            }
            for (label, url) in &actor.links {
                let Some(url) = url.as_str() else {
                    continue;
                };
                let Some(link_bytes) = label.len().checked_add(url.len()) else {
                    return Ok(None);
                };
                if !include(sequence_label_text_work_units(link_bytes, false))? {
                    return Ok(None);
                }
            }
        }
    }
    for message in &model.messages {
        if !include(sequence_message_text_work_units(message))? {
            return Ok(None);
        }
    }
    for sequence_box in &model.boxes {
        if let Some(name) = sequence_box.name.as_deref()
            && !include(sequence_label_text_work_units(name.len(), false))?
        {
            return Ok(None);
        }
    }
    if let Some(title) = sequence_render_title(model.title.as_deref(), diagram_title)
        && !include(sequence_label_text_work_units(title.len(), false))?
    {
        return Ok(None);
    }

    checkpoints.checkpoint()?;
    Ok(Some(work_units))
}

#[cfg(all(test, feature = "diagram-sequence"))]
fn sequence_layout_work_units(model: &SequenceDiagramModel) -> Option<usize> {
    let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
    sequence_layout_work_units_controlled(model, None, SequenceLayoutCheckpoints::new(&meter))
        .ok()
        .flatten()
}

fn sequence_layout_work_units_controlled(
    model: &SequenceDiagramModel,
    diagram_title: Option<&str>,
    checkpoints: SequenceLayoutCheckpoints<'_>,
) -> Result<Option<usize>> {
    let mut box_memberships = 0usize;
    let mut membership_index = 0usize;
    for sequence_box in &model.boxes {
        for _ in &sequence_box.actor_keys {
            checkpoints.checkpoint_loop(membership_index)?;
            membership_index = membership_index.saturating_add(1);
            let Some(next) = box_memberships.checked_add(1) else {
                return Ok(None);
            };
            box_memberships = next;
        }
    }
    checkpoints.checkpoint()?;

    let mut popup_links = 0usize;
    for (actor_position, actor_id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(actor_position)?;
        let actor_links = model
            .actors
            .get(actor_id)
            .map_or(0, |actor| actor.links.len());
        let Some(next) = popup_links.checked_add(actor_links) else {
            return Ok(None);
        };
        popup_links = next;
    }
    checkpoints.checkpoint()?;

    let Some(text_work) = sequence_text_work_units_controlled(model, diagram_title, checkpoints)?
    else {
        return Ok(None);
    };

    Ok(SequenceLayoutWorkShape {
        actors: model.actor_order.len(),
        messages: model.messages.len(),
        boxes: model.boxes.len(),
        box_memberships,
        popup_links,
        text_work,
    }
    .work_units())
}

pub(crate) fn bracketize_sequence_block_label(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "\u{200B}".to_string();
    }
    format!("[{trimmed}]")
}

pub(crate) fn sequence_block_label_wrap_width(block_width: f64, wrap_padding: f64) -> f64 {
    (block_width - 2.0 * wrap_padding).max(1.0)
}

pub(crate) fn sequence_block_widths_for_render(
    model: &SequenceDiagramModel,
    prepared: &SequencePreparedArtifact,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    work_meter: &OperationWorkMeter,
) -> Result<FxHashMap<String, f64>> {
    let checkpoints = SequenceOperationCheckpoints::for_emit(work_meter);
    checkpoints.checkpoint()?;
    let settings = SequenceLayoutSettings::from_effective_config(
        effective_config.as_value(),
        prepared.typography(),
    );
    // SVG frame emission reconstructs Mermaid's `calculateLoopBounds` after Rust layout has
    // already completed. Only the built-in operation route may carry its earlier message bounds
    // across this private split; host and custom routes deliberately replay every callback.
    let message_metrics = prepared
        .message_metrics
        .view(model, &settings.msg_text_style, measurer);
    let mut actor_index = HashMap::with_capacity(model.actor_order.len());
    for (actor_position, actor_id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(actor_position)?;
        actor_index.insert(actor_id.as_str(), actor_position);
    }
    let mut actor_centers_x = Vec::with_capacity(model.actor_order.len());
    let mut actor_widths = Vec::with_capacity(model.actor_order.len());
    for (actor_position, actor_id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(actor_position)?;
        let node_id = format!("actor-top-{actor_id}");
        let Some(node) = nodes_by_id.get(node_id.as_str()).copied() else {
            return Ok(FxHashMap::default());
        };
        actor_centers_x.push(node.x);
        actor_widths.push(node.width);
    }

    let widths = calculate_sequence_block_widths(BlockStepPlanContext {
        model,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_widths: &actor_widths,
        actor_margin: settings.actor_margin,
        activation_width: settings.activation_width,
        box_margin: settings.box_margin,
        box_text_margin: settings.box_text_margin,
        label_box_height: prepared.block_label_box_metrics().layout_height(),
        label_box_width: prepared.block_label_box_metrics().width(),
        sequence_default_width: settings.sequence_default_width,
        wrap_padding: settings.wrap_padding,
        note_margin: settings.note_margin,
        is_neo: settings.is_neo,
        measurer,
        msg_text_style: &settings.msg_text_style,
        note_text_style: &settings.note_text_style,
        note_non_normal_typed_terminal_style: prepared
            .typography()
            .note()
            .non_normal_typed_terminal_style(),
        loop_text_style: &settings.loop_text_style,
        math_config: effective_config,
        math_terminal_style: prepared
            .typography()
            .terminal_text_style(SequenceTextSurface::ControlPrimaryTitle),
        note_math_terminal_style: prepared
            .typography()
            .terminal_text_style(SequenceTextSurface::NoteLabel),
        math_sidecar: prepared.math_sidecar(),
        message_metrics,
        checkpoints,
    })?;
    let mut widths_by_id = FxHashMap::default();
    for (index, (id, width)) in widths.into_iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        widths_by_id.insert(id, width);
    }
    checkpoints.checkpoint()?;
    Ok(widths_by_id)
}

/// Prepares a Sequence model under the cumulative work meter owned by the render operation.
#[allow(
    clippy::too_many_arguments,
    reason = "Prepared rendering keeps theme, text, math, and work inputs explicit"
)]
pub(crate) fn prepare_sequence_diagram_typed_with_title_and_work_meter(
    model: &SequenceDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &PresentationConfig,
    resolved_theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>,
    measurer: &dyn TextMeasurer,
    math_backend: Option<&crate::math::ConfiguredMathBackend>,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<SequencePreparedArtifact> {
    let checkpoints = SequenceLayoutCheckpoints::new(work_meter.as_ref());
    checkpoints.checkpoint()?;
    work_meter.preflight_sequence_complexity(model, OperationPhase::Layout)?;
    checkpoints.checkpoint()?;
    let work_units = sequence_layout_work_units_controlled(model, diagram_title, checkpoints)?
        .ok_or_else(|| work_meter.arithmetic_overflow())?;
    work_meter.charge(work_units)?;

    let source_actor_order = model.actor_order.as_slice();
    let visible_model =
        if effective_config.get_bool("sequence.hideUnusedParticipants") == Some(true) {
            sequence_visible_model(model, checkpoints)?
        } else {
            None
        };
    let model = visible_model.as_deref().unwrap_or(model);

    let typography = Arc::new(SequenceTypographyPlan::resolve(
        effective_config,
        resolved_theme,
        work_meter.as_ref(),
    )?);
    checkpoints.checkpoint()?;
    let terminal_theme = SequencePreparedTerminalTheme::resolve(
        model,
        effective_config,
        resolved_theme,
        work_meter.as_ref(),
    )?;
    let effective_title = sequence_render_title(model.title.as_deref(), diagram_title);
    let theme_evidence = SequenceThemeEvidenceRecorder::with_title(
        resolved_theme,
        effective_config,
        effective_title,
        work_meter.as_ref(),
    )?;
    let base_text_style = typography.base_measurement_style();
    let effective_config_value = effective_config.as_value();
    let settings =
        SequenceLayoutSettings::from_effective_config(effective_config_value, typography.as_ref());
    let math_sidecar_builder = SequenceMathSidecarBuilder::new(
        math_backend,
        effective_config,
        measurer,
        Arc::clone(&work_meter),
    )
    .with_actor_terminal_emissions(if settings.mirror_actors { 2 } else { 1 });
    let block_label_box_metrics = SequenceBlockLabelBoxMetrics::resolve(
        model,
        settings.label_box_width,
        settings.label_box_height,
        settings.box_text_margin,
        measurer,
        typography.loop_label(),
        checkpoints,
    )?;
    checkpoints.checkpoint()?;

    let SequenceActorLayoutPlan {
        actor_index,
        actor_widths,
        actor_popup_layouts,
        actor_base_heights,
        actor_label_footprints,
        actor_text_heights,
        actor_centers_x,
        box_layouts,
        box_title_height,
        actor_top_offset_y,
        max_actor_layout_height,
        has_boxes,
        message_metrics,
    } = plan_sequence_actors(SequenceActorLayoutPlanContext {
        model,
        source_actor_order,
        measurer,
        actor_text_style: &settings.actor_text_style,
        note_text_style: &settings.note_text_style,
        msg_text_style: &settings.msg_text_style,
        math_config: effective_config,
        typography: typography.as_ref(),
        math_sidecar: &math_sidecar_builder,
        actor_width_min: settings.sequence_default_width,
        actor_height: settings.actor_height,
        is_neo: settings.is_neo,
        actor_margin: settings.actor_margin,
        label_box_height: settings.label_box_height,
        box_margin: settings.box_margin,
        box_text_margin: settings.box_text_margin,
        wrap_padding: settings.wrap_padding,
        note_margin: settings.note_margin,
        checkpoints,
    })?;
    let message_metric_view = message_metrics.view(model, &settings.msg_text_style, measurer);

    let clusters: Vec<LayoutCluster> = Vec::new();

    let SequenceLayoutGraph {
        mut nodes,
        wrapped_note_text,
        edges,
        block_layouts_by_id,
        activations,
        activation_geometry,
        bottom_box_top_y,
        box_height,
        bounds_start_x,
        bounds_stop_x,
    } = build_sequence_layout_graph(SequenceLayoutGraphContext {
        model,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_widths: &actor_widths,
        actor_base_heights: &actor_base_heights,
        actor_label_footprints: &actor_label_footprints,
        actor_text_heights: &actor_text_heights,
        actor_top_offset_y,
        max_actor_layout_height,
        sequence_default_width: settings.sequence_default_width,
        actor_height: settings.actor_height,
        actor_margin: settings.actor_margin,
        box_margin: settings.box_margin,
        note_margin: settings.note_margin,
        box_text_margin: settings.box_text_margin,
        label_box_height: settings.label_box_height,
        block_label_box_height: block_label_box_metrics.layout_height(),
        block_label_box_width: block_label_box_metrics.width(),
        right_angles: settings.right_angles,
        is_neo: settings.is_neo,
        wrap_padding: settings.wrap_padding,
        mirror_actors: settings.mirror_actors,
        activation_width: settings.activation_width,
        measurer,
        msg_text_style: &settings.msg_text_style,
        msg_terminal_text_style: typography.message().terminal_text_style(),
        note_text_style: &settings.note_text_style,
        note_terminal_text_style: typography.note().terminal_text_style(),
        loop_text_style: &settings.loop_text_style,
        math_config: effective_config,
        typography: typography.as_ref(),
        math_sidecar: &math_sidecar_builder,
        message_metrics: message_metric_view,
        checkpoints,
    })?;

    let wrapped_note_text = SequencePreparedNoteLabels::new(
        model,
        wrapped_note_text,
        &SequenceWrappedSideNoteLabelContext {
            measurer,
            measurement_style: &settings.note_text_style,
            terminal_style: typography.note().terminal_text_style(),
            config: effective_config,
            default_width: settings.sequence_default_width,
            note_margin: settings.note_margin,
            wrap_padding: settings.wrap_padding,
            checkpoints: checkpoints.text(),
        },
        &work_meter,
    )?;

    let rect_x_bounds = sequence_rect_stack_x_bounds(SequenceRectStackBoundsContext {
        model,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        edges: &edges,
        nodes: &nodes,
        actor_width_min: settings.sequence_default_width,
        box_margin: settings.box_margin,
        checkpoints,
    })?;
    if !rect_x_bounds.is_empty() {
        for (node_index, n) in nodes.iter_mut().enumerate() {
            checkpoints.checkpoint_loop(node_index)?;
            let Some(start_id) = n.id.strip_prefix("rect-") else {
                continue;
            };
            let Some((min_x, max_x)) = rect_x_bounds.get(start_id).copied() else {
                continue;
            };
            n.x = (min_x + max_x) / 2.0;
            n.width = (max_x - min_x).max(1.0);
        }
    }

    let SequenceFrameGeometry {
        frames,
        placements: frame_placements,
    } = prepare_sequence_frame_geometry(
        model,
        &nodes,
        &edges,
        &block_layouts_by_id,
        block_label_box_metrics.width(),
        checkpoints,
    )?;

    let mut boxes = Vec::with_capacity(box_layouts.len());
    let mut box_indices = Vec::with_capacity(box_layouts.len());
    let padding = settings.box_margin * 2.0;
    for (index, (source, actor_box)) in model.boxes.iter().zip(&box_layouts).enumerate() {
        checkpoints.checkpoint_loop(index)?;
        let Some(actor_x) = actor_box.x else {
            box_indices.push(None);
            continue;
        };
        let x = actor_x - padding;
        let width = actor_box.width + padding * 2.0;
        box_indices.push(Some(boxes.len()));
        boxes.push(SequenceParticipantBoxLayout {
            x,
            y: -padding * 0.25,
            width,
            height: box_height + padding * 0.75,
            fill: source.fill.clone(),
            title: actor_box
                .label
                .as_deref()
                .filter(|label| !label.is_empty())
                .map(|text| SequenceTextLayout {
                    text: text.to_owned(),
                    x: x + width / 2.0,
                    y: settings.box_text_margin + box_title_height / 2.0,
                }),
        });
    }

    let SequenceRootGeometry {
        bounds,
        diagram_title,
    } = prepare_sequence_root_geometry(SequenceRootBoundsContext {
        model,
        diagram_title: effective_title,
        nodes: &nodes,
        edges: &edges,
        frames: &frames,
        bounds_start_x,
        bounds_stop_x,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_popup_layouts: &actor_popup_layouts,
        box_layouts: &box_layouts,
        actor_width_min: settings.sequence_default_width,
        actor_height: settings.actor_height,
        bottom_box_top_y,
        diagram_margin_x: settings.diagram_margin_x,
        diagram_margin_y: settings.diagram_margin_y,
        bottom_margin_adj: settings.bottom_margin_adj,
        box_margin: settings.box_margin,
        has_boxes,
        mirror_actors: settings.mirror_actors,
        measurer,
        base_text_style: &base_text_style,
        msg_text_style: &settings.msg_text_style,
        math_config: effective_config,
        math_sidecar: &math_sidecar_builder,
        message_metrics: message_metric_view,
        block_label_box_metrics,
        checkpoints,
    })?;

    let math_sidecar = math_sidecar_builder.finish()?;
    checkpoints.checkpoint()?;
    let activation_theme = SequencePreparedStaticRectTheme::resolve(
        resolved_theme,
        crate::diagram_theme::ThemeTarget::Activation,
        !activations.is_empty(),
        effective_config.config_paint_overrides_typed_default("themeVariables.activationBkgColor"),
        effective_config
            .config_paint_overrides_typed_default("themeVariables.activationBorderColor"),
        work_meter.as_ref(),
    )?;

    let css = SequencePreparedCss::resolve(&typography, &terminal_theme);
    Ok(SequencePreparedArtifact {
        visible_model,
        layout: SequenceDiagramLayout {
            nodes,
            edges,
            clusters,
            bounds: Some(bounds),
            frames,
            activations,
            boxes,
            title: diagram_title,
        },
        actor_popup_layouts,
        actor_label_footprints,
        wrapped_note_text,
        message_metrics,
        math_sidecar,
        theme_evidence,
        effect_evidence: Default::default(),
        expected_effect_applications: Default::default(),
        typography,
        terminal_theme,
        activation_geometry,
        activation_theme,
        css,
        block_label_box_metrics,
        box_indices,
        frame_placements,
    })
}

fn sequence_visible_model(
    model: &SequenceDiagramModel,
    checkpoints: SequenceLayoutCheckpoints<'_>,
) -> Result<Option<Box<SequenceDiagramModel>>> {
    // Mermaid counts endpoints from every message, including notes and activation records.
    let mut used = rustc_hash::FxHashSet::default();
    for (index, message) in model.messages.iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        used.extend(message.from.as_deref());
        used.extend(message.to.as_deref());
    }
    let mut hidden = false;
    for (index, id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        hidden |= !used.contains(id.as_str());
    }
    if !hidden {
        return Ok(None);
    }

    // Retain the message allocation with its metric sidecar through layout and SVG emission.
    // Source actor order and parser-owned lifecycle data remain untouched.
    let mut visible = Box::new(model.clone());
    visible.actor_order.retain(|id| used.contains(id.as_str()));
    if let Some(lifecycles) = &mut visible.actor_lifecycles {
        let mut index = 0;
        lifecycles.retain(|_| {
            let keep = model
                .actor_order
                .get(index)
                .is_some_and(|id| used.contains(id.as_str()));
            index += 1;
            keep
        });
    }
    checkpoints.checkpoint()?;
    Ok(Some(visible))
}

pub(crate) fn sequence_render_title<'a>(
    model_title: Option<&'a str>,
    diagram_title: Option<&'a str>,
) -> Option<&'a str> {
    if model_title.is_none_or(|t| t.trim().is_empty())
        && let Some(title) = diagram_title.map(str::trim).filter(|t| !t.is_empty())
    {
        return Some(title);
    }
    model_title
}

#[cfg(all(test, feature = "diagram-sequence"))]
mod resource_tests {
    use super::{
        SEQUENCE_MESSAGE_LAYOUT_WORK_UNITS, SequenceLayoutWorkShape,
        prepare_sequence_diagram_typed_with_title_and_work_meter, sequence_block_widths_for_render,
        sequence_label_text_work_units, sequence_layout_work_units,
        sequence_message_text_work_units,
    };
    use crate::Error;
    use crate::resources::{
        OperationWorkMeter, RenderResourcePolicy, ResourceLimitCause, ResourceLimitId,
    };
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle};
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::presentation::RenderSemanticModel;
    use merman_model::{OperationControl, OperationPhase};
    use rustc_hash::FxHashMap;
    use serde_json::json;
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;
    use std::fmt::Write;
    use std::sync::Arc;

    #[derive(Default)]
    struct CountingTextMeasurer {
        calls: Cell<usize>,
        inner: DeterministicTextMeasurer,
    }

    impl TextMeasurer for CountingTextMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.calls.set(self.calls.get() + 1);
            self.inner.measure(text, style)
        }
    }

    struct CancellingTextMeasurer {
        texts_after_cancellation: RefCell<HashSet<String>>,
        trigger_occurrences: Cell<usize>,
        trigger: String,
        cancel_after_occurrence: usize,
        control: OperationControl,
        inner: DeterministicTextMeasurer,
    }

    impl TextMeasurer for CancellingTextMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            if self.control.is_cancelled() {
                self.texts_after_cancellation
                    .borrow_mut()
                    .insert(text.to_string());
            }
            if text == self.trigger {
                let occurrences = self.trigger_occurrences.get() + 1;
                self.trigger_occurrences.set(occurrences);
                if occurrences == self.cancel_after_occurrence {
                    self.control.cancel();
                }
            }
            self.inner.measure(text, style)
        }
    }

    fn sequence_model(source: &str) -> merman_model::families::sequence::SequenceDiagramModel {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected Sequence model");
        };
        model.clone()
    }

    #[test]
    fn wrapped_note_artifact_reserves_exact_bytes_and_releases_on_late_preparation_failure() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, FontStyle, Specified, TextStylePatch,
            ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
        };

        const LATE_TITLE: &str = "cancel after note retention";

        struct CancelAtTitle {
            meter: Arc<OperationWorkMeter>,
            title_calls: Cell<usize>,
            pre_retention_probes: Cell<usize>,
        }
        impl TextMeasurer for CancelAtTitle {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                DeterministicTextMeasurer::default().measure(text, style)
            }

            fn measure_svg_title_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
                if text != LATE_TITLE {
                    if self.meter.prepared_text_retained_bytes() == 0 {
                        self.pre_retention_probes
                            .set(self.pre_retention_probes.get() + 1);
                    }
                    return DeterministicTextMeasurer::default()
                        .measure_svg_title_bbox_x(text, style);
                }
                self.title_calls.set(self.title_calls.get() + 1);
                assert!(self.meter.prepared_text_retained_bytes() > 0);
                self.meter.control().cancel();
                (0.0, 0.0)
            }
        }

        let source = "---\nconfig: {sequence: {wrap: true}}\n---\nsequenceDiagram\nparticipant A\nNote right of A: alpha beta gamma";
        let model = sequence_model(source);
        let config = crate::test_support::presentation_config(json!({"sequence": {"wrap": true}}));
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::NoteLabel,
                        ThemeStylePatch {
                            typography: TextStylePatch {
                                font_style: Specified::Value(FontStyle::Italic),
                                ..TextStylePatch::default()
                            },
                            ..ThemeStylePatch::default()
                        },
                    ),
                )),
            )
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::SEQUENCE);
        let measurer = DeterministicTextMeasurer::default();
        let prepare = |meter: &Arc<OperationWorkMeter>| {
            prepare_sequence_diagram_typed_with_title_and_work_meter(
                &model,
                None,
                &config,
                Some(&resolved),
                &measurer,
                None,
                Arc::clone(meter),
            )
        };
        let measuring = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let artifact = prepare(&measuring).unwrap();
        let exact_bytes = measuring.prepared_text_retained_bytes();
        assert!(exact_bytes > "alpha beta gamma".len());
        drop(artifact);
        assert_eq!(measuring.prepared_text_retained_bytes(), 0);

        for ceiling in [1, exact_bytes - 1, exact_bytes] {
            let meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, ceiling)
                    .unwrap(),
            ));
            if ceiling == exact_bytes {
                let artifact = prepare(&meter).unwrap();
                assert_eq!(meter.prepared_text_retained_bytes(), exact_bytes);
                drop(artifact);
            } else {
                let Error::ResourceLimitExceeded(error) = prepare(&meter).unwrap_err() else {
                    panic!("expected retained Note text to respect the prepared-text byte ceiling");
                };
                assert_eq!(
                    error.limit,
                    ResourceLimitId::MaxPreparedTextRetainedBytes.as_str()
                );
                assert_eq!(error.cause, ResourceLimitCause::Ceiling);
                assert_eq!(error.max, ceiling);
            }
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }

        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let cancelling_measurer = CancelAtTitle {
            meter: Arc::clone(&meter),
            title_calls: Cell::new(0),
            pre_retention_probes: Cell::new(0),
        };
        let error = prepare_sequence_diagram_typed_with_title_and_work_meter(
            &model,
            Some(LATE_TITLE),
            &config,
            Some(&resolved),
            &cancelling_measurer,
            None,
            Arc::clone(&meter),
        )
        .unwrap_err();
        assert!(matches!(error, Error::Cancelled(error) if error.phase == OperationPhase::Layout));
        assert!(cancelling_measurer.pre_retention_probes.get() > 0);
        assert_eq!(cancelling_measurer.title_calls.get(), 1);
        assert!(meter.prepared_text_retained_bytes_peak() > 0);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn nested_sequence_frames_have_linear_layout_work() {
        let flat = sequence_model("sequenceDiagram\nA->>B: hi\n");
        let nested =
            sequence_model("sequenceDiagram\nloop outer\nloop inner\nA->>B: hi\nend\nend\n");
        let flat_text_work = flat
            .messages
            .iter()
            .map(|message| sequence_message_text_work_units(message).unwrap())
            .sum::<usize>();
        let nested_text_work = nested
            .messages
            .iter()
            .map(|message| sequence_message_text_work_units(message).unwrap())
            .sum::<usize>();

        assert_eq!(
            sequence_layout_work_units(&nested).unwrap()
                - sequence_layout_work_units(&flat).unwrap(),
            (nested.messages.len() - flat.messages.len()) * SEQUENCE_MESSAGE_LAYOUT_WORK_UNITS
                + nested_text_work
                - flat_text_work
        );
    }

    #[test]
    fn control_labels_budget_the_renderer_owned_brackets_and_wrapping() {
        let model = sequence_model("sequenceDiagram\nloop a long control label\nA->>B: hi\nend\n");
        let control = model
            .messages
            .iter()
            .find(|message| message.control_semantics().is_some())
            .expect("loop control record");
        assert!(!control.wrap);

        let bracketed_bytes = control.message_text().len() + 2;
        assert_eq!(
            sequence_message_text_work_units(control),
            sequence_label_text_work_units(bracketed_bytes, true)
        );
    }

    #[test]
    fn invisible_control_records_do_not_budget_label_measurement_work() {
        let model = sequence_model(
            "sequenceDiagram\nrect rgb(240,240,240)\nA->>B: hi\nend\nloop visible\nA->>B: hi\nend\n",
        );
        let controls = model
            .messages
            .iter()
            .filter_map(|message| {
                message
                    .control_semantics()
                    .map(|semantics| (message, semantics))
            })
            .collect::<Vec<_>>();

        assert!(controls.len() >= 4);
        for (message, semantics) in controls {
            let work = sequence_message_text_work_units(message)
                .expect("control label work should remain representable");
            if !semantics.consumes_text() {
                assert_eq!(work, 0, "{semantics:?}");
            } else {
                assert!(work > 0, "{semantics:?}");
            }
        }
    }

    #[test]
    fn actor_link_labels_and_urls_are_included_in_sequence_work() {
        let without_link = sequence_model("sequenceDiagram\nparticipant A\n");
        let with_link = sequence_model(
            "sequenceDiagram\nparticipant A\nlink A: Documentation @ https://example.test/docs\n",
        );

        assert!(
            sequence_layout_work_units(&with_link).unwrap()
                > sequence_layout_work_units(&without_link).unwrap()
        );
    }

    #[test]
    fn participant_only_sequence_is_reported_and_budgeted() {
        let model = sequence_model(include_str!(
            "../../../fixtures/sequence/upstream_pkgtests_sequencediagram_spec_094.mmd"
        ));
        let expected_work = sequence_layout_work_units(&model).unwrap();
        assert!(expected_work > 0);

        let measurer = CountingTextMeasurer::default();
        let narrow_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, expected_work - 1)
            .unwrap();
        let narrow_meter = Arc::new(OperationWorkMeter::new(narrow_policy));
        let error = prepare_sequence_diagram_typed_with_title_and_work_meter(
            &model,
            None,
            &crate::test_support::presentation_config(json!({})),
            None,
            &measurer,
            None,
            Arc::clone(&narrow_meter),
        )
        .unwrap_err();
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected layout work resource error");
        };
        assert_eq!(error.cause, ResourceLimitCause::Ceiling);
        assert_eq!(error.actual, expected_work);
        assert_eq!(narrow_meter.used(), 0);
        assert_eq!(measurer.calls.get(), 0);

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, expected_work)
            .unwrap();
        let exact_meter = Arc::new(OperationWorkMeter::new(exact_policy));
        prepare_sequence_diagram_typed_with_title_and_work_meter(
            &model,
            None,
            &crate::test_support::presentation_config(json!({})),
            None,
            &measurer,
            None,
            Arc::clone(&exact_meter),
        )
        .unwrap();
        assert_eq!(exact_meter.used(), expected_work);
        assert!(measurer.calls.get() > 0);
    }

    #[test]
    fn wrapped_sequence_text_is_admitted_before_measurement_callbacks() {
        let mut model = sequence_model("sequenceDiagram\nparticipant A\n");
        let actor = model.actors.get_mut("A").expect("participant A");
        actor.description = "x".repeat(130);
        actor.wrap = true;
        let expected_work = sequence_layout_work_units(&model).unwrap();

        let measurer = CountingTextMeasurer::default();
        let effective_config = crate::test_support::presentation_config(json!({}));
        let narrow_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, expected_work - 1)
            .unwrap();
        let narrow_meter = Arc::new(OperationWorkMeter::new(narrow_policy));
        let error = prepare_sequence_diagram_typed_with_title_and_work_meter(
            &model,
            None,
            &effective_config,
            None,
            &measurer,
            None,
            Arc::clone(&narrow_meter),
        )
        .unwrap_err();
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected layout work resource error");
        };

        assert_eq!(error.actual, expected_work);
        assert_eq!(narrow_meter.used(), 0);
        assert_eq!(measurer.calls.get(), 0);

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, expected_work)
            .unwrap();
        let exact_meter = Arc::new(OperationWorkMeter::new(exact_policy));
        prepare_sequence_diagram_typed_with_title_and_work_meter(
            &model,
            None,
            &effective_config,
            None,
            &measurer,
            None,
            Arc::clone(&exact_meter),
        )
        .unwrap();

        assert_eq!(exact_meter.used(), expected_work);
        assert!(measurer.calls.get() > 0);
    }

    #[test]
    fn duplicate_participants_follow_the_normalized_mermaid_actor_order() {
        let single = sequence_model(include_str!(
            "../../../fixtures/sequence/upstream_pkgtests_sequencediagram_spec_094.mmd"
        ));
        let repeated = sequence_model(include_str!(
            "../../../fixtures/sequence/upstream_duplicate_participants_single_actor_spec.mmd"
        ));

        assert_eq!(single.actor_order, repeated.actor_order);
        assert_eq!(
            sequence_layout_work_units(&single),
            sequence_layout_work_units(&repeated)
        );
    }

    #[test]
    fn sequence_svg_frame_width_reconstruction_observes_mid_emit_cancellation() {
        const SIGNAL_COUNT: usize = 130;

        let mut source =
            String::from("sequenceDiagram\nparticipant A\nparticipant B\nloop outer\n");
        for index in 0..SIGNAL_COUNT {
            let label = if index == 0 {
                "trigger".to_string()
            } else {
                format!("message-{index}")
            };
            writeln!(&mut source, "A->>B: $${label}$$").unwrap();
        }
        source.push_str("end\n");
        let model = sequence_model(&source);
        let effective_config = crate::test_support::presentation_config(json!({}));
        let preparation_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let prepared = prepare_sequence_diagram_typed_with_title_and_work_meter(
            &model,
            None,
            &effective_config,
            None,
            &DeterministicTextMeasurer::default(),
            None,
            Arc::clone(&preparation_meter),
        )
        .unwrap();

        let control = OperationControl::new();
        let measurer = CancellingTextMeasurer {
            texts_after_cancellation: RefCell::new(HashSet::new()),
            trigger_occurrences: Cell::new(0),
            trigger: "$$trigger$$".to_string(),
            cancel_after_occurrence: 1,
            control: control.clone(),
            inner: DeterministicTextMeasurer::default(),
        };
        let work_meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        );
        let nodes_by_id: FxHashMap<&str, &crate::model::LayoutNode> = prepared
            .layout()
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();

        let error = sequence_block_widths_for_render(
            &model,
            &prepared,
            &nodes_by_id,
            &crate::test_support::presentation_config(json!({})),
            &measurer,
            &work_meter,
        )
        .unwrap_err();
        let Error::Cancelled(error) = error else {
            panic!("expected Sequence frame planning cancellation");
        };

        assert_eq!(error.phase, OperationPhase::Emit);
        assert!(measurer.trigger_occurrences.get() >= 1);
        assert!(
            measurer.texts_after_cancellation.borrow().len() < 64,
            "frame planning exceeded the cooperative checkpoint cadence after cancellation"
        );
        assert_eq!(work_meter.used(), 0);
    }

    #[test]
    fn actor_and_box_membership_work_curves_are_linear() {
        for actors in [1usize, 32, 1_024] {
            assert_eq!(
                SequenceLayoutWorkShape {
                    actors,
                    messages: 0,
                    boxes: 0,
                    box_memberships: 0,
                    popup_links: 0,
                    text_work: 0,
                }
                .work_units(),
                Some(actors * super::SEQUENCE_ACTOR_LAYOUT_WORK_UNITS)
            );
        }

        let one_membership = SequenceLayoutWorkShape {
            actors: 2,
            messages: 0,
            boxes: 1,
            box_memberships: 1,
            popup_links: 0,
            text_work: 0,
        }
        .work_units()
        .unwrap();
        let many_memberships = SequenceLayoutWorkShape {
            actors: 2,
            messages: 0,
            boxes: 1,
            box_memberships: 65,
            popup_links: 0,
            text_work: 0,
        }
        .work_units()
        .unwrap();
        assert_eq!(
            many_memberships - one_membership,
            64 * super::SEQUENCE_BOX_MEMBERSHIP_WORK_UNITS
        );

        let popup_links = SequenceLayoutWorkShape {
            actors: 1,
            messages: 0,
            boxes: 0,
            box_memberships: 0,
            popup_links: 32,
            text_work: 0,
        }
        .work_units()
        .unwrap();
        assert_eq!(
            popup_links,
            super::SEQUENCE_ACTOR_LAYOUT_WORK_UNITS
                + 32 * super::SEQUENCE_POPUP_LINK_LAYOUT_WORK_UNITS
        );
    }

    #[test]
    fn sequence_layout_work_shape_fails_closed_on_overflow() {
        assert_eq!(
            SequenceLayoutWorkShape {
                actors: usize::MAX,
                messages: 0,
                boxes: 0,
                box_memberships: 0,
                popup_links: 0,
                text_work: 0,
            }
            .work_units(),
            None
        );
    }
}
