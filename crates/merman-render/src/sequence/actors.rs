use super::constants::sequence_actor_lifeline_start_y;
use super::constants::sequence_actor_visual_height;
use crate::model::{LayoutEdge, LayoutNode, LayoutPoint};
use merman_core::diagrams::sequence::SequenceActor;
use std::collections::{BTreeMap, HashMap};

pub(super) struct SequenceActorLifecycleContext<'a> {
    pub(super) actor_index: &'a HashMap<&'a str, usize>,
    pub(super) actor_widths: &'a [f64],
    pub(super) actor_base_heights: &'a [f64],
    pub(super) actors: &'a BTreeMap<String, SequenceActor>,
    pub(super) created_actors: &'a BTreeMap<String, usize>,
    pub(super) destroyed_actors: &'a BTreeMap<String, usize>,
    pub(super) actor_height: f64,
    pub(super) actor_width_min: f64,
    pub(super) label_box_height: f64,
}

pub(super) struct SequenceActorLifecycle<'a> {
    ctx: SequenceActorLifecycleContext<'a>,
    created_top_center_y: BTreeMap<String, f64>,
    destroyed_bottom_top_y: BTreeMap<String, f64>,
}

pub(super) struct SequenceFooterActorContext<'a, 'b> {
    pub(super) actor_order: &'a [String],
    pub(super) actors: &'a BTreeMap<String, SequenceActor>,
    pub(super) actor_widths: &'a [f64],
    pub(super) actor_centers_x: &'a [f64],
    pub(super) actor_base_heights: &'a [f64],
    pub(super) actor_lifecycle: &'b SequenceActorLifecycle<'a>,
    pub(super) actor_top_offset_y: f64,
    pub(super) bottom_box_top_y: f64,
    pub(super) mirror_actors: bool,
    pub(super) label_box_height: f64,
    pub(super) box_text_margin: f64,
}

pub(super) struct SequenceTopActorContext<'a> {
    pub(super) actor_order: &'a [String],
    pub(super) actors: &'a BTreeMap<String, SequenceActor>,
    pub(super) actor_widths: &'a [f64],
    pub(super) actor_centers_x: &'a [f64],
    pub(super) actor_base_heights: &'a [f64],
    pub(super) actor_top_offset_y: f64,
    pub(super) label_box_height: f64,
}

impl<'a> SequenceActorLifecycle<'a> {
    pub(super) fn new(ctx: SequenceActorLifecycleContext<'a>) -> Self {
        Self {
            ctx,
            created_top_center_y: BTreeMap::new(),
            destroyed_bottom_top_y: BTreeMap::new(),
        }
    }

    pub(super) fn created_actor_index(&self, actor_id: &str) -> Option<usize> {
        self.ctx.created_actors.get(actor_id).copied()
    }

    pub(super) fn destroyed_actor_index(&self, actor_id: &str) -> Option<usize> {
        self.ctx.destroyed_actors.get(actor_id).copied()
    }

    pub(super) fn created_top_center_y(&self, actor_id: &str) -> Option<f64> {
        self.created_top_center_y.get(actor_id).copied()
    }

    pub(super) fn destroyed_bottom_top_y(&self, actor_id: &str) -> Option<f64> {
        self.destroyed_bottom_top_y.get(actor_id).copied()
    }

    pub(super) fn apply_message_y_adjustment(
        &mut self,
        msg_idx: usize,
        from: &str,
        to: &str,
        line_y: f64,
    ) -> f64 {
        // Mermaid updates created/destroyed actor vertical anchors while processing messages and
        // advances the cursor by half of the affected actor's visual height.
        if self.created_actor_index(to) == Some(msg_idx) {
            let h = self.actor_visual_height(to);
            self.created_top_center_y.insert(to.to_string(), line_y);
            h / 2.0
        } else if self.destroyed_actor_index(from) == Some(msg_idx) {
            let h = self.actor_visual_height(from);
            self.destroyed_bottom_top_y
                .insert(from.to_string(), line_y - h / 2.0);
            h / 2.0
        } else if self.destroyed_actor_index(to) == Some(msg_idx) {
            let h = self.actor_visual_height(to);
            self.destroyed_bottom_top_y
                .insert(to.to_string(), line_y - h / 2.0);
            h / 2.0
        } else {
            0.0
        }
    }

    pub(super) fn apply_created_top_actor_positions(&self, nodes: &mut [LayoutNode]) {
        // Created actors render their top box at the creation message y-position after the full
        // message cursor pass has discovered that position.
        for node in nodes {
            let Some(actor_id) = node.id.strip_prefix("actor-top-") else {
                continue;
            };
            if let Some(y) = self.created_top_center_y(actor_id) {
                node.y = y;
            }
        }
    }

    fn actor_visual_height(&self, actor_id: &str) -> f64 {
        let Some(idx) = self.ctx.actor_index.get(actor_id).copied() else {
            return self.ctx.actor_height.max(1.0);
        };
        let w = self
            .ctx
            .actor_widths
            .get(idx)
            .copied()
            .unwrap_or(self.ctx.actor_width_min);
        let base_h = self
            .ctx
            .actor_base_heights
            .get(idx)
            .copied()
            .unwrap_or(self.ctx.actor_height);
        self.ctx
            .actors
            .get(actor_id)
            .map(|a| a.actor_type.as_str())
            .map(|t| sequence_actor_visual_height(t, w, base_h, self.ctx.label_box_height))
            .unwrap_or(base_h.max(1.0))
    }
}

pub(super) fn sequence_actor_is_type_width_limited(
    actors: &BTreeMap<String, SequenceActor>,
    actor_id: &str,
) -> bool {
    actors
        .get(actor_id)
        .map(|a| {
            matches!(
                a.actor_type.as_str(),
                "actor" | "control" | "entity" | "database"
            )
        })
        .unwrap_or(false)
}

pub(super) fn append_sequence_top_actors(
    nodes: &mut Vec<LayoutNode>,
    ctx: SequenceTopActorContext<'_>,
) {
    for (idx, id) in ctx.actor_order.iter().enumerate() {
        let w = ctx.actor_widths[idx];
        let cx = ctx.actor_centers_x[idx];
        let base_h = ctx.actor_base_heights[idx];
        let actor_type = ctx
            .actors
            .get(id)
            .map(|a| a.actor_type.as_str())
            .unwrap_or("participant");
        let visual_h = sequence_actor_visual_height(actor_type, w, base_h, ctx.label_box_height);
        let top_y = ctx.actor_top_offset_y + visual_h / 2.0;
        nodes.push(LayoutNode {
            id: format!("actor-top-{id}"),
            x: cx,
            y: top_y,
            width: w,
            height: visual_h,
            is_cluster: false,
            label_width: None,
            label_height: None,
        });
    }
}

pub(super) fn append_sequence_footer_actors(
    nodes: &mut Vec<LayoutNode>,
    edges: &mut Vec<LayoutEdge>,
    ctx: SequenceFooterActorContext<'_, '_>,
) {
    for (idx, id) in ctx.actor_order.iter().enumerate() {
        let w = ctx.actor_widths[idx];
        let cx = ctx.actor_centers_x[idx];
        let base_h = ctx.actor_base_heights[idx];
        let actor_type = ctx
            .actors
            .get(id)
            .map(|a| a.actor_type.as_str())
            .unwrap_or("participant");
        let visual_h = sequence_actor_visual_height(actor_type, w, base_h, ctx.label_box_height);
        let bottom_top_y = ctx
            .actor_lifecycle
            .destroyed_bottom_top_y(id)
            .unwrap_or(ctx.bottom_box_top_y);
        let bottom_visual_h = if ctx.mirror_actors { visual_h } else { 0.0 };
        nodes.push(LayoutNode {
            id: format!("actor-bottom-{id}"),
            x: cx,
            y: bottom_top_y + bottom_visual_h / 2.0,
            width: w,
            height: bottom_visual_h,
            is_cluster: false,
            label_width: None,
            label_height: None,
        });

        let top_center_y = ctx
            .actor_lifecycle
            .created_top_center_y(id)
            .unwrap_or(ctx.actor_top_offset_y + visual_h / 2.0);
        let top_left_y = top_center_y - visual_h / 2.0;
        let lifeline_start_y =
            top_left_y + sequence_actor_lifeline_start_y(actor_type, base_h, ctx.box_text_margin);

        edges.push(LayoutEdge {
            id: format!("lifeline-{id}"),
            from: format!("actor-top-{id}"),
            to: format!("actor-bottom-{id}"),
            from_cluster: None,
            to_cluster: None,
            points: vec![
                LayoutPoint {
                    x: cx,
                    y: lifeline_start_y,
                },
                LayoutPoint {
                    x: cx,
                    y: bottom_top_y,
                },
            ],
            label: None,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }
}
