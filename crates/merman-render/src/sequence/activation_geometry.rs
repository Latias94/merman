use super::{SequenceLayoutCheckpoints, sequence_activation_start_x};
use crate::model::{LayoutEdge, LayoutNode};
use merman_core::diagrams::sequence::{SequenceDiagramRenderModel, SequenceMessageKind};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
struct SequenceActivationStart {
    startx: f64,
    starty: f64,
    group_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct SequenceActivationRect {
    pub(crate) startx: f64,
    pub(crate) starty: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) class_idx: usize,
    pub(crate) actor_index: usize,
}

#[derive(Debug)]
pub(crate) struct SequencePreparedActivationGeometry {
    pub(crate) groups: Vec<Option<SequenceActivationRect>>,
    pub(crate) group_by_start_id: FxHashMap<String, usize>,
    rect_count: usize,
}
impl SequencePreparedActivationGeometry {
    pub(super) fn prepare(
        model: &SequenceDiagramRenderModel,
        nodes: &[LayoutNode],
        edges: &[LayoutEdge],
        activation_width: f64,
        checkpoints: SequenceLayoutCheckpoints<'_>,
    ) -> crate::Result<Self> {
        let mut node_index = FxHashMap::default();
        for (index, node) in nodes.iter().enumerate() {
            checkpoints.checkpoint_loop(index)?;
            node_index.insert(node.id.as_str(), node);
        }
        let mut edge_index = FxHashMap::default();
        for (index, edge) in edges.iter().enumerate() {
            checkpoints.checkpoint_loop(index)?;
            edge_index.insert(edge.id.as_str(), edge);
        }
        let nodes_by_id = &node_index;
        let edges_by_id = &edge_index;
        let mut actor_indexes =
            FxHashMap::with_capacity_and_hasher(model.actor_order.len(), Default::default());
        for (actor_index, actor_id) in model.actor_order.iter().enumerate() {
            checkpoints.checkpoint_loop(actor_index)?;
            actor_indexes.insert(actor_id.as_str(), actor_index);
        }

        let mut last_line_y: Option<f64> = None;
        let mut activation_stacks: std::collections::BTreeMap<&str, Vec<SequenceActivationStart>> =
            std::collections::BTreeMap::new();
        let mut groups: Vec<Option<SequenceActivationRect>> = Vec::new();
        let mut rect_count = 0usize;
        let mut group_by_start_id: FxHashMap<String, usize> =
            FxHashMap::with_capacity_and_hasher(model.messages.len(), Default::default());

        for (message_index, msg) in model.messages.iter().enumerate() {
            checkpoints.checkpoint_loop(message_index)?;
            if let Some(y) = msg_line_y(edges_by_id, &msg.id) {
                last_line_y = Some(y);
            }

            match msg.semantic_kind() {
                SequenceMessageKind::ActivationStart => {
                    let Some(actor_id) = msg.from.as_deref() else {
                        continue;
                    };
                    let Some(cx) = actor_center_x(nodes_by_id, actor_id) else {
                        continue;
                    };
                    let has_any_activation = !activation_stacks.is_empty();
                    let stack = activation_stacks.entry(actor_id).or_default();
                    let stacked_size = stack.len();
                    let startx = sequence_activation_start_x(cx, stacked_size, activation_width);

                    let starty = last_line_y
                        .or_else(|| lifeline_y(edges_by_id, actor_id).map(|(y0, _y1)| y0))
                        .unwrap_or(0.0);
                    let starty = if last_line_y.is_some() && has_any_activation {
                        starty + 2.0
                    } else {
                        starty
                    };

                    let group_index = groups.len();
                    groups.push(None);
                    group_by_start_id.insert(msg.id.clone(), group_index);
                    stack.push(SequenceActivationStart {
                        startx,
                        starty,
                        group_index,
                    });
                }
                SequenceMessageKind::ActivationEnd => {
                    let Some(actor_id) = msg.from.as_deref() else {
                        continue;
                    };
                    let Some(stack) = activation_stacks.get_mut(actor_id) else {
                        continue;
                    };
                    let Some(start) = stack.pop() else {
                        continue;
                    };

                    let mut starty = start.starty;
                    let mut vertical_pos = last_line_y.unwrap_or(starty);
                    if starty + 18.0 > vertical_pos {
                        starty = vertical_pos - 6.0;
                        vertical_pos += 12.0;
                    }

                    let class_idx = stack.len() % 3;
                    let rect = SequenceActivationRect {
                        startx: start.startx,
                        starty,
                        width: activation_width,
                        height: (vertical_pos - starty).max(0.0),
                        class_idx,
                        actor_index: actor_indexes.get(actor_id).copied().unwrap_or(0),
                    };
                    if let Some(slot) = groups.get_mut(start.group_index) {
                        *slot = Some(rect);
                        rect_count = rect_count.saturating_add(1);
                    }
                }
                _ => {}
            }

            let _ = msg.activate;
        }

        checkpoints.checkpoint()?;
        Ok(Self {
            groups,
            group_by_start_id,
            rect_count,
        })
    }
    pub(crate) const fn rect_count(&self) -> usize {
        self.rect_count
    }
}
fn actor_center_x(nodes_by_id: &FxHashMap<&str, &LayoutNode>, actor_id: &str) -> Option<f64> {
    let node_id = format!("actor-top-{actor_id}");
    nodes_by_id.get(node_id.as_str()).copied().map(|n| n.x)
}

fn lifeline_y(
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    actor_id: &str,
) -> Option<(f64, f64)> {
    let edge_id = format!("lifeline-{actor_id}");
    let e = edges_by_id.get(edge_id.as_str()).copied()?;
    let y0 = e.points.first()?.y;
    let y1 = e.points.last()?.y;
    Some((y0, y1))
}

fn msg_line_y(
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    msg_id: &str,
) -> Option<f64> {
    let edge_id = format!("msg-{msg_id}");
    let e = edges_by_id.get(edge_id.as_str()).copied()?;
    Some(e.points.first()?.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};

    #[test]
    fn prepared_activation_geometry_keeps_unmatched_groups_and_requires_actor_geometry() {
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nA->>+B: Hello\nB-->>-A: Reply",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let merman_core::RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected Sequence model");
        };
        let work = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let prepared = super::super::prepare_sequence_diagram_typed_with_title_and_work_meter(
            model,
            None,
            &merman_core::MermaidConfig::default(),
            None,
            None,
            &crate::text::DeterministicTextMeasurer::default(),
            None,
            std::sync::Arc::clone(&work),
        )
        .unwrap();
        assert_eq!(prepared.activation_geometry().rect_count(), 1);
        let mut unmatched = model.clone();
        unmatched
            .messages
            .retain(|message| message.semantic_kind() != SequenceMessageKind::ActivationEnd);
        let checkpoints = SequenceLayoutCheckpoints::new(work.as_ref());
        let geometry = SequencePreparedActivationGeometry::prepare(
            &unmatched,
            &prepared.layout().nodes,
            &prepared.layout().edges,
            10.0,
            checkpoints,
        )
        .unwrap();
        assert_eq!(geometry.rect_count(), 0);
        assert_eq!(geometry.groups.len(), 1);
        assert!(geometry.groups[0].is_none());
        assert_eq!(geometry.group_by_start_id.len(), 1);
        let geometry = SequencePreparedActivationGeometry::prepare(
            model,
            &[],
            &prepared.layout().edges,
            10.0,
            checkpoints,
        )
        .unwrap();
        assert_eq!(geometry.rect_count(), 0);
        assert!(geometry.groups.is_empty());
    }
}
