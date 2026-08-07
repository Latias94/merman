use crate::Result;
use crate::math::MathRenderer;
use crate::model::{LayoutCluster, SequenceDiagramLayout};
use crate::resources::OperationWorkMeter;
use crate::text::TextMeasurer;
use merman_core::MermaidConfig;
use merman_core::diagrams::sequence::SequenceDiagramRenderModel;
use rustc_hash::FxHashMap;
use serde_json::Value;
use std::collections::HashMap;

const SEQUENCE_ACTOR_LAYOUT_WORK_UNITS: usize = 12;
const SEQUENCE_MESSAGE_LAYOUT_WORK_UNITS: usize = 8;
const SEQUENCE_BOX_LAYOUT_WORK_UNITS: usize = 3;
const SEQUENCE_BOX_MEMBERSHIP_WORK_UNITS: usize = 2;

mod activation;
mod actors;
mod block_steps;
pub(crate) mod config;
mod constants;
mod messages;
mod metrics;
mod notes;
mod orchestration;
mod rect;
mod root_bounds;

pub(crate) use activation::{sequence_activation_stack_bounds, sequence_activation_start_x};
pub(crate) use constants::{
    SEQUENCE_FRAME_GEOM_PAD_PX, SEQUENCE_FRAME_SIDE_PAD_PX, SEQUENCE_MESSAGE_WRAP_PADDING_SIDES,
    SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX, sequence_actor_popup_panel_height,
    sequence_text_dimensions_height_px, sequence_text_line_step_px,
};
pub(crate) use metrics::{
    SequenceMathHeightMode, measure_sequence_math_label, wrap_sequence_label_like_mermaid_lines,
};
pub(crate) use notes::sequence_note_final_wrapped_lines;

use actors::{SequenceActorLayoutPlan, SequenceActorLayoutPlanContext, plan_sequence_actors};
use block_steps::{BlockStepPlanContext, calculate_sequence_block_widths};
use config::SequenceLayoutSettings;
use orchestration::{SequenceLayoutGraph, SequenceLayoutGraphContext, build_sequence_layout_graph};
use rect::sequence_rect_stack_x_bounds;
use root_bounds::{SequenceRootBoundsContext, sequence_root_bounds};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceLayoutWorkShape {
    actors: usize,
    messages: usize,
    boxes: usize,
    box_memberships: usize,
}

impl SequenceLayoutWorkShape {
    fn from_model(model: &SequenceDiagramRenderModel) -> Option<Self> {
        let box_memberships = model.boxes.iter().try_fold(0usize, |total, sequence_box| {
            total.checked_add(sequence_box.actor_keys.len())
        })?;
        Some(Self {
            actors: model.actor_order.len(),
            messages: model.messages.len(),
            boxes: model.boxes.len(),
            box_memberships,
        })
    }

    fn work_units(self) -> Option<usize> {
        // Sequence layout has a fixed number of actor and message passes: measurement, spacing,
        // geometry construction, frame propagation, and final bounds. Box membership is scanned
        // twice by Mermaid-compatible actor margin and actor-to-box assignment. Nested frame
        // accumulators are propagated to their parent once, so block depth does not multiply the
        // per-message term.
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
            )
    }
}

fn sequence_layout_work_units(model: &SequenceDiagramRenderModel) -> Option<usize> {
    SequenceLayoutWorkShape::from_model(model)?.work_units()
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
    model: &SequenceDiagramRenderModel,
    layout: &SequenceDiagramLayout,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> FxHashMap<String, f64> {
    let settings = SequenceLayoutSettings::from_effective_config(effective_config.as_value());
    let nodes_by_id: HashMap<&str, _> = layout
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let actor_index: HashMap<&str, usize> = model
        .actor_order
        .iter()
        .enumerate()
        .map(|(index, actor_id)| (actor_id.as_str(), index))
        .collect();
    let mut actor_centers_x = Vec::with_capacity(model.actor_order.len());
    let mut actor_widths = Vec::with_capacity(model.actor_order.len());
    for actor_id in &model.actor_order {
        let node_id = format!("actor-top-{actor_id}");
        let Some(node) = nodes_by_id.get(node_id.as_str()).copied() else {
            return FxHashMap::default();
        };
        actor_centers_x.push(node.x);
        actor_widths.push(node.width);
    }

    calculate_sequence_block_widths(BlockStepPlanContext {
        model,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_widths: &actor_widths,
        actor_margin: settings.actor_margin,
        activation_width: settings.activation_width,
        box_margin: settings.box_margin,
        box_text_margin: settings.box_text_margin,
        label_box_height: settings.label_box_height,
        label_box_width: settings.label_box_width,
        sequence_default_width: settings.sequence_default_width,
        wrap_padding: settings.wrap_padding,
        note_margin: settings.note_margin,
        is_neo: settings.is_neo,
        measurer,
        msg_text_style: &settings.msg_text_style,
        note_text_style: &settings.note_text_style,
        math_config: effective_config,
        math_renderer,
    })
    .into_iter()
    .collect()
}

/// Lays out a Sequence model under the cumulative work meter owned by the render operation.
pub(crate) fn layout_sequence_diagram_typed_with_title_and_work_meter(
    model: &SequenceDiagramRenderModel,
    diagram_title: Option<&str>,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work_meter: &OperationWorkMeter,
) -> Result<SequenceDiagramLayout> {
    work_meter.policy().check_sequence_complexity(model)?;
    let work_units =
        sequence_layout_work_units(model).ok_or_else(|| work_meter.arithmetic_overflow())?;
    work_meter.charge(work_units)?;

    let math_config = MermaidConfig::from_value(effective_config.clone());
    let settings = SequenceLayoutSettings::from_effective_config(effective_config);

    let SequenceActorLayoutPlan {
        actor_index,
        actor_widths,
        actor_base_heights,
        actor_box,
        actor_left_x,
        actor_centers_x,
        box_margins,
        actor_top_offset_y,
        max_actor_layout_height,
        has_boxes,
    } = plan_sequence_actors(SequenceActorLayoutPlanContext {
        model,
        measurer,
        actor_text_style: &settings.actor_text_style,
        note_text_style: &settings.note_text_style,
        msg_text_style: &settings.msg_text_style,
        math_config: &math_config,
        math_renderer,
        actor_width_min: settings.sequence_default_width,
        actor_height: settings.actor_height,
        actor_margin: settings.actor_margin,
        actor_font_size: settings.actor_text_style.font_size,
        box_margin: settings.box_margin,
        box_text_margin: settings.box_text_margin,
        wrap_padding: settings.wrap_padding,
        message_font_size: settings.msg_text_style.font_size,
    })?;

    let clusters: Vec<LayoutCluster> = Vec::new();

    let SequenceLayoutGraph {
        mut nodes,
        edges,
        block_layouts_by_id,
        bottom_box_top_y,
        bounds_start_x,
        bounds_stop_x,
    } = build_sequence_layout_graph(SequenceLayoutGraphContext {
        model,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_widths: &actor_widths,
        actor_base_heights: &actor_base_heights,
        actor_top_offset_y,
        max_actor_layout_height,
        sequence_default_width: settings.sequence_default_width,
        actor_height: settings.actor_height,
        actor_margin: settings.actor_margin,
        box_margin: settings.box_margin,
        note_margin: settings.note_margin,
        box_text_margin: settings.box_text_margin,
        label_box_height: settings.label_box_height,
        label_box_width: settings.label_box_width,
        right_angles: settings.right_angles,
        is_neo: settings.is_neo,
        wrap_padding: settings.wrap_padding,
        mirror_actors: settings.mirror_actors,
        activation_width: settings.activation_width,
        measurer,
        msg_text_style: &settings.msg_text_style,
        note_text_style: &settings.note_text_style,
        math_config: &math_config,
        math_renderer,
    });

    let rect_x_bounds = sequence_rect_stack_x_bounds(
        model,
        &actor_index,
        &actor_centers_x,
        &edges,
        &nodes,
        settings.sequence_default_width,
        settings.box_margin,
    );
    if !rect_x_bounds.is_empty() {
        for n in &mut nodes {
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

    let bounds = Some(sequence_root_bounds(SequenceRootBoundsContext {
        model,
        diagram_title,
        nodes: &nodes,
        edges: &edges,
        bounds_start_x,
        bounds_stop_x,
        actor_index: &actor_index,
        actor_centers_x: &actor_centers_x,
        actor_left_x: &actor_left_x,
        actor_widths: &actor_widths,
        actor_box: &actor_box,
        box_margins: &box_margins,
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
        msg_text_style: &settings.msg_text_style,
        math_config: &math_config,
        math_renderer,
    }));

    Ok(SequenceDiagramLayout {
        nodes,
        edges,
        clusters,
        bounds,
        block_layouts_by_id,
    })
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

#[cfg(test)]
mod resource_tests {
    use super::{
        SEQUENCE_MESSAGE_LAYOUT_WORK_UNITS, SequenceLayoutWorkShape,
        layout_sequence_diagram_typed_with_title_and_work_meter, sequence_layout_work_units,
    };
    use crate::Error;
    use crate::resources::{
        OperationWorkMeter, RenderResourcePolicy, ResourceLimitCause, ResourceLimitId,
    };
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle};
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};
    use serde_json::json;
    use std::cell::Cell;

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

    fn sequence_model(source: &str) -> merman_core::diagrams::sequence::SequenceDiagramRenderModel {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected Sequence model");
        };
        model.clone()
    }

    #[test]
    fn nested_sequence_frames_have_linear_layout_work() {
        let flat = sequence_model("sequenceDiagram\nA->>B: hi\n");
        let nested =
            sequence_model("sequenceDiagram\nloop outer\nloop inner\nA->>B: hi\nend\nend\n");

        assert_eq!(
            sequence_layout_work_units(&nested).unwrap()
                - sequence_layout_work_units(&flat).unwrap(),
            (nested.messages.len() - flat.messages.len()) * SEQUENCE_MESSAGE_LAYOUT_WORK_UNITS
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
        let narrow_meter = OperationWorkMeter::new(narrow_policy);
        let error = layout_sequence_diagram_typed_with_title_and_work_meter(
            &model,
            None,
            &json!({}),
            &measurer,
            None,
            &narrow_meter,
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
        let exact_meter = OperationWorkMeter::new(exact_policy);
        layout_sequence_diagram_typed_with_title_and_work_meter(
            &model,
            None,
            &json!({}),
            &measurer,
            None,
            &exact_meter,
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
    fn actor_and_box_membership_work_curves_are_linear() {
        for actors in [1usize, 32, 1_024] {
            assert_eq!(
                SequenceLayoutWorkShape {
                    actors,
                    messages: 0,
                    boxes: 0,
                    box_memberships: 0,
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
        }
        .work_units()
        .unwrap();
        let many_memberships = SequenceLayoutWorkShape {
            actors: 2,
            messages: 0,
            boxes: 1,
            box_memberships: 65,
        }
        .work_units()
        .unwrap();
        assert_eq!(
            many_memberships - one_membership,
            64 * super::SEQUENCE_BOX_MEMBERSHIP_WORK_UNITS
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
            }
            .work_units(),
            None
        );
    }
}
