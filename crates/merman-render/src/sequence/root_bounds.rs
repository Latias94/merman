use super::SequenceLayoutCheckpoints;
use super::block_collection::{SequenceBlock, collect_sequence_blocks};
use super::block_geometry::{frame_x_from_actors, resolved_block_frame_x};
use super::constants::sequence_actor_popup_panel_height;
use super::message_metrics::{SequenceMessageMetricView, SequenceMessageOwner};
use super::metrics::{SequenceMathHeightMode, measure_sequence_label_for_layout_with_prepared};
use crate::Result;
use crate::model::{Bounds, LayoutEdge, LayoutNode, SequenceBlockLayout};
use crate::text::{TextMeasurer, TextStyle};
use merman_core::MermaidConfig;
use merman_core::diagrams::sequence::{SequenceDiagramRenderModel, SequenceMessageKind};
use rustc_hash::FxHashMap;
use std::collections::HashMap;

pub(super) struct SequenceRootBoundsContext<'a> {
    pub(super) model: &'a SequenceDiagramRenderModel,
    pub(super) diagram_title: Option<&'a str>,
    pub(super) nodes: &'a [LayoutNode],
    pub(super) edges: &'a [LayoutEdge],
    pub(super) block_layouts_by_id: &'a FxHashMap<String, SequenceBlockLayout>,
    pub(super) bounds_start_x: f64,
    pub(super) bounds_stop_x: f64,
    pub(super) actor_index: &'a HashMap<&'a str, usize>,
    pub(super) actor_centers_x: &'a [f64],
    pub(super) actor_left_x: &'a [f64],
    pub(super) actor_widths: &'a [f64],
    pub(super) actor_popup_widths: &'a HashMap<String, f64>,
    pub(super) actor_box: &'a [Option<usize>],
    pub(super) box_margins: &'a [f64],
    pub(super) actor_width_min: f64,
    pub(super) actor_height: f64,
    pub(super) bottom_box_top_y: f64,
    pub(super) diagram_margin_x: f64,
    pub(super) diagram_margin_y: f64,
    pub(super) bottom_margin_adj: f64,
    pub(super) box_margin: f64,
    pub(super) has_boxes: bool,
    pub(super) mirror_actors: bool,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) base_text_style: &'a TextStyle,
    pub(super) msg_text_style: &'a TextStyle,
    pub(super) math_config: &'a MermaidConfig,
    pub(super) math_sidecar: &'a super::SequenceMathSidecarBuilder<'a>,
    pub(super) message_metrics: SequenceMessageMetricView<'a>,
    pub(super) block_label_box_metrics: super::SequenceBlockLabelBoxMetrics,
    pub(super) checkpoints: SequenceLayoutCheckpoints<'a>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SequenceDiagramTitleGeometry {
    text: String,
    x: f64,
    y: f64,
    bounds: Bounds,
}

impl SequenceDiagramTitleGeometry {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) const fn x(&self) -> f64 {
        self.x
    }

    pub(crate) const fn y(&self) -> f64 {
        self.y
    }
}

pub(super) struct SequenceRootGeometry {
    pub(super) bounds: Bounds,
    pub(super) diagram_title: Option<SequenceDiagramTitleGeometry>,
}

pub(super) fn prepare_sequence_root_geometry(
    ctx: SequenceRootBoundsContext<'_>,
) -> Result<SequenceRootGeometry> {
    let mut content = sequence_content_bounds(&ctx)?;

    include_actor_popup_bounds(&mut content, &ctx)?;

    // Mermaid (11.12.2) expands the viewBox vertically when a sequence title is present.
    // See `sequenceRenderer.ts`: `extraVertForTitle = title ? 40 : 0`.
    let extra_vert_for_title = if ctx.diagram_title.is_some() {
        40.0
    } else {
        0.0
    };

    // Mermaid's sequence renderer sets the viewBox y origin to
    // `-(diagramMarginY + extraVertForTitle)` regardless of diagram contents.
    let vb_min_y = -(ctx.diagram_margin_y + extra_vert_for_title);

    // Mermaid's sequence renderer uses a bounds box with `starty = 0` and computes `height` from
    // `stopy - starty`. Our headless layout models message spacing in content coordinates, but for
    // viewBox parity we must follow the upstream formula.
    let mut bounds_box_stopy = if ctx.mirror_actors {
        content.max_y + ctx.bottom_margin_adj
    } else {
        content.max_y
    }
    .max(0.0);

    // When boxes exist, Mermaid's bounds logic extends the vertical bounds by `boxMargin`
    // (diagramMarginY covers the remaining box padding), so include it here.
    if ctx.has_boxes {
        bounds_box_stopy += ctx.box_margin;
    }

    let mut bounds_box = ActorHorizontalBounds::from_content(ctx.bounds_start_x, ctx.bounds_stop_x);
    bounds_box.include(content.min_x, content.max_x);
    bounds_box.include_actor_boxes(&ctx)?;
    include_self_message_bounds(&mut bounds_box, &ctx)?;
    include_resolved_block_label_box_bounds(&mut bounds_box, &ctx)?;
    ctx.checkpoints.checkpoint()?;

    let diagram_title = prepare_diagram_title_geometry(&ctx, &bounds_box)?;
    let mut bounds = Bounds {
        min_x: bounds_box.start_x - ctx.diagram_margin_x,
        min_y: vb_min_y,
        max_x: bounds_box.stop_x + ctx.diagram_margin_x,
        max_y: bounds_box_stopy + ctx.diagram_margin_y,
    };
    if let Some(title) = diagram_title.as_ref() {
        bounds.min_x = bounds.min_x.min(title.bounds.min_x);
        bounds.min_y = bounds.min_y.min(title.bounds.min_y);
        bounds.max_x = bounds.max_x.max(title.bounds.max_x);
        bounds.max_y = bounds.max_y.max(title.bounds.max_y);
    }

    Ok(SequenceRootGeometry {
        bounds,
        diagram_title,
    })
}

fn prepare_diagram_title_geometry(
    ctx: &SequenceRootBoundsContext<'_>,
    bounds_box: &ActorHorizontalBounds,
) -> Result<Option<SequenceDiagramTitleGeometry>> {
    let Some(text) = ctx.diagram_title else {
        return Ok(None);
    };
    let x = (bounds_box.stop_x - bounds_box.start_x) / 2.0 - 2.0 * ctx.diagram_margin_x;
    let y = -25.0;
    ctx.checkpoints.checkpoint()?;
    let (left, right) = ctx
        .measurer
        .measure_svg_title_bbox_x(text, ctx.base_text_style);
    ctx.checkpoints.checkpoint()?;
    let (ascent, descent) = crate::text::svg_title_bbox_vertical_extents_px(ctx.base_text_style);

    Ok(Some(SequenceDiagramTitleGeometry {
        text: text.to_owned(),
        x,
        y,
        bounds: Bounds {
            min_x: x - left,
            min_y: y - ascent,
            max_x: x + right,
            max_y: y + descent,
        },
    }))
}

fn include_resolved_block_label_box_bounds(
    bounds_box: &mut ActorHorizontalBounds,
    ctx: &SequenceRootBoundsContext<'_>,
) -> Result<()> {
    if !ctx.block_label_box_metrics.typography_expanded() {
        return Ok(());
    }

    // Mermaid anchors each label box to the left edge of its resolved control-block frame. Consume
    // the same family-owned block plan as terminal SVG emission instead of guessing that edge from
    // a global actor position.
    let mut nodes_by_id = FxHashMap::default();
    for (node_index, node) in ctx.nodes.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(node_index)?;
        nodes_by_id.insert(node.id.as_str(), node);
    }
    let mut edges_by_id = FxHashMap::default();
    for (edge_index, edge) in ctx.edges.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(edge_index)?;
        edges_by_id.insert(edge.id.as_str(), edge);
    }
    let mut actor_nodes_by_id = FxHashMap::default();
    for (actor_index, actor_id) in ctx.model.actor_order.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(actor_index)?;
        let node_id = format!("actor-top-{actor_id}");
        if let Some(node) = nodes_by_id.get(node_id.as_str()).copied() {
            actor_nodes_by_id.insert(actor_id.as_str(), node);
        }
    }

    let Some(default_frame) = frame_x_from_actors(ctx.model, &nodes_by_id, ctx.checkpoints)? else {
        return Ok(());
    };
    let (_, blocks) = collect_sequence_blocks(
        ctx.model,
        &actor_nodes_by_id,
        &edges_by_id,
        &nodes_by_id,
        ctx.block_layouts_by_id,
        ctx.checkpoints,
    )?;
    for (block_index, block) in blocks.into_iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(block_index)?;
        let Some(layout) = block.layout() else {
            continue;
        };
        let critical_section_count = match &block {
            SequenceBlock::Critical { sections, .. } => Some(sections.len()),
            _ => None,
        };
        let frame = resolved_block_frame_x(
            block.geometry(),
            layout,
            &actor_nodes_by_id,
            default_frame,
            critical_section_count,
        );
        let Some((frame_left, frame_right, _)) = frame else {
            continue;
        };
        bounds_box.include(
            frame_left,
            frame_right.max(frame_left + ctx.block_label_box_metrics.width()),
        );
    }
    Ok(())
}

fn sequence_content_bounds(ctx: &SequenceRootBoundsContext<'_>) -> Result<ContentBounds> {
    let mut content = ContentBounds::new();

    for (node_index, n) in ctx.nodes.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(node_index)?;
        let left = n.x - n.width / 2.0;
        let right = n.x + n.width / 2.0;
        let bottom = n.y + n.height / 2.0;
        content.include_x(left, right);
        // When `mirrorActors=false`, Mermaid does not draw footer actor boxes. The internal
        // bottom actor placeholders still anchor lifelines in our layout, but upstream root
        // sizing ignores those invisible placeholders and uses message/popup geometry instead.
        if ctx.mirror_actors || !n.id.starts_with("actor-bottom-") {
            content.include_y(bottom);
        }
    }

    include_footer_row_height(&mut content, ctx)?;

    if !ctx.mirror_actors {
        for (edge_index, e) in ctx.edges.iter().enumerate() {
            ctx.checkpoints.checkpoint_loop(edge_index)?;
            if e.id.starts_with("lifeline-") {
                continue;
            }
            for (point_index, p) in e.points.iter().enumerate() {
                ctx.checkpoints.checkpoint_loop(point_index)?;
                content.include_y(p.y);
            }
            if let Some(label) = e.label.as_ref() {
                content.include_y(label.y + label.height / 2.0);
            }
        }
    }

    Ok(content.or_fallback(
        ctx.actor_width_min.max(1.0),
        (ctx.bottom_box_top_y + ctx.actor_height).max(1.0),
    ))
}

fn include_footer_row_height(
    content: &mut ContentBounds,
    ctx: &SequenceRootBoundsContext<'_>,
) -> Result<()> {
    if !ctx.mirror_actors {
        return Ok(());
    }

    // Mermaid's footer draw pass bumps the shared bounds cursor by the maximum rendered actor
    // height for the whole footer row, even when some actors were destroyed earlier and have their
    // own `stopy`. The root viewport follows that cursor, not just each individual footer node.
    let mut max_footer_height = 0.0_f64;
    for (node_index, node) in ctx.nodes.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(node_index)?;
        if node.id.starts_with("actor-bottom-") {
            max_footer_height = max_footer_height.max(node.height);
        }
    }
    if max_footer_height > 0.0 {
        content.include_y(ctx.bottom_box_top_y + max_footer_height);
    }
    Ok(())
}

fn include_actor_popup_bounds(
    content: &mut ContentBounds,
    ctx: &SequenceRootBoundsContext<'_>,
) -> Result<()> {
    // Mermaid's root `getBBox()` still includes actor popup menu panels when links/directives are
    // present, even when they are emitted hidden by default. Account for the measured menu width
    // and panel bottom so root bounds follow the same ActorLabel typography as terminal SVG.
    for (actor_index, actor_id) in ctx.model.actor_order.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(actor_index)?;
        let Some(actor) = ctx.model.actors.get(actor_id) else {
            continue;
        };
        if actor.links.is_empty() {
            continue;
        }
        let panel_left = ctx.actor_left_x.get(actor_index).copied().unwrap_or(0.0);
        let panel_width = ctx
            .actor_popup_widths
            .get(actor_id)
            .copied()
            .or_else(|| ctx.actor_widths.get(actor_index).copied())
            .unwrap_or(ctx.actor_width_min);
        content.include_x(panel_left, panel_left + panel_width);
        let popup_bottom = ctx.actor_height + sequence_actor_popup_panel_height(actor.links.len());
        let popup_content_bottom = if ctx.mirror_actors {
            popup_bottom - ctx.diagram_margin_y - if ctx.has_boxes { ctx.box_margin } else { 0.0 }
        } else {
            popup_bottom
        };
        content.include_y(popup_content_bottom.max(0.0));
    }
    Ok(())
}

fn include_self_message_bounds(
    bounds_box: &mut ActorHorizontalBounds,
    ctx: &SequenceRootBoundsContext<'_>,
) -> Result<()> {
    // Mermaid's self-message bounds insert expands horizontally by
    // `dx = max(textWidth/2, conf.width/2)`, where `conf.width` is the configured actor width
    // (150 by default). This can increase `box.stopx` by ~1px due to `from_x + 1` rounding
    // behavior in message geometry, affecting viewBox width.
    for (message_index, msg) in ctx.model.messages.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(message_index)?;
        let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
            continue;
        };
        if from != to {
            continue;
        }
        // Notes can use `from==to` for `rightOf`/`leftOf`; ignore them here.
        if msg.semantic_kind() == SequenceMessageKind::Note {
            continue;
        }
        let Some(&i) = ctx.actor_index.get(from) else {
            continue;
        };
        let center_x = ctx.actor_centers_x[i] + 1.0;
        let text = msg.message_text();
        let premeasured_bound = ctx
            .message_metrics
            .get(SequenceMessageOwner::from_model_index(message_index), msg);
        let (text_w, _text_h) = if text.is_empty() {
            (1.0, 1.0)
        } else if let Some(metrics) = premeasured_bound {
            (metrics.width(), 0.0)
        } else {
            let prepared_math = ctx
                .math_sidecar
                .get(&super::SequenceMathOccurrence::Message(message_index), text);
            measure_sequence_label_for_layout_with_prepared(
                prepared_math.as_deref(),
                ctx.measurer,
                text,
                ctx.msg_text_style,
                ctx.math_config,
                None,
                SequenceMathHeightMode::Bound,
                ctx.checkpoints.text(),
            )?
        };
        let dx = (text_w.max(1.0) / 2.0).max(ctx.actor_width_min / 2.0);
        bounds_box.include(center_x - dx, center_x + dx);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ContentBounds {
    min_x: f64,
    max_x: f64,
    max_y: f64,
}

impl ContentBounds {
    fn new() -> Self {
        Self {
            min_x: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }

    fn include_x(&mut self, left: f64, right: f64) {
        self.min_x = self.min_x.min(left);
        self.max_x = self.max_x.max(right);
    }

    fn include_y(&mut self, y: f64) {
        self.max_y = self.max_y.max(y);
    }

    fn or_fallback(mut self, fallback_width: f64, fallback_max_y: f64) -> Self {
        if !self.min_x.is_finite() {
            self.min_x = 0.0;
            self.max_x = fallback_width;
            self.max_y = fallback_max_y;
        }
        self
    }
}

struct ActorHorizontalBounds {
    start_x: f64,
    stop_x: f64,
}

impl ActorHorizontalBounds {
    fn from_content(min_x: f64, max_x: f64) -> Self {
        Self {
            start_x: min_x,
            stop_x: max_x,
        }
    }

    fn include_actor_boxes(&mut self, ctx: &SequenceRootBoundsContext<'_>) -> Result<()> {
        // Mermaid's bounds box includes the per-box inner margins (`box.margin`) when boxes exist.
        // Approximate this by extending actor bounds by their enclosing box margin.
        for i in 0..ctx.model.actor_order.len() {
            ctx.checkpoints.checkpoint_loop(i)?;
            let left = ctx.actor_left_x[i];
            let right = left + ctx.actor_widths[i];
            if let Some(bi) = ctx.actor_box[i] {
                let m = ctx.box_margins[bi];
                self.include(left - m, right + m);
            } else {
                self.include(left, right);
            }
        }
        Ok(())
    }

    fn include(&mut self, left: f64, right: f64) {
        self.start_x = self.start_x.min(left);
        self.stop_x = self.stop_x.max(right);
    }
}
