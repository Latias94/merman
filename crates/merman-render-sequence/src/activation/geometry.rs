use super::sequence_activation_start_x;
use crate::frame::SEQUENCE_CHECKPOINT_INTERVAL;
use crate::model::SequenceActivationLayout;
use merman_model::OperationPhase;
use merman_render_core::{Result, resources::OperationWorkMeter};
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct SequenceActivationStart {
    actor: String,
    startx: f64,
    starty: f64,
}

#[derive(Debug, Clone)]
pub struct SequenceActivationGroup {
    pub layout_index: usize,
    pub actor_index: usize,
}

#[derive(Debug, Default)]
pub struct SequencePreparedActivationGeometry {
    pub(super) groups: Vec<Option<SequenceActivationGroup>>,
    group_by_start_id: FxHashMap<String, usize>,
}

impl SequencePreparedActivationGeometry {
    /// Distinguishes an open activation's placeholder from a message with no activation.
    pub fn group_for_start(&self, message_id: &str) -> Option<Option<&SequenceActivationGroup>> {
        let group_index = *self.group_by_start_id.get(message_id)?;
        Some(self.groups.get(group_index).and_then(Option::as_ref))
    }
}

pub struct SequenceActivationGeometryBuilder {
    width: f64,
    stacks: BTreeMap<usize, Vec<usize>>,
    active: BTreeMap<usize, SequenceActivationStart>,
    groups: Vec<Option<(SequenceActivationLayout, usize)>>,
    group_by_start_id: FxHashMap<String, usize>,
    rect_count: usize,
}

impl SequenceActivationGeometryBuilder {
    pub fn new(width: f64) -> Self {
        Self {
            width,
            stacks: BTreeMap::new(),
            active: BTreeMap::new(),
            groups: Vec::new(),
            group_by_start_id: FxHashMap::default(),
            rect_count: 0,
        }
    }

    pub fn start(
        &mut self,
        message_id: &str,
        actor: &str,
        actor_index: usize,
        center_x: f64,
        cursor_y: f64,
    ) {
        let stack = self.stacks.entry(actor_index).or_default();
        let group_index = self.groups.len();
        self.active.insert(
            group_index,
            SequenceActivationStart {
                actor: actor.to_owned(),
                startx: sequence_activation_start_x(center_x, stack.len(), self.width),
                starty: cursor_y + 2.0,
            },
        );
        stack.push(group_index);
        self.groups.push(None);
        self.group_by_start_id
            .insert(message_id.to_string(), group_index);
    }

    pub fn end(&mut self, actor_index: usize, cursor_y: f64) -> Option<&SequenceActivationLayout> {
        let stack = self.stacks.get_mut(&actor_index)?;
        let group_index = stack.pop()?;
        let start = self.active.remove(&group_index)?;
        let mut starty = start.starty;
        let mut stop_y = cursor_y;
        if starty + 18.0 > stop_y {
            starty = stop_y - 6.0;
            stop_y += 12.0;
        }
        let layout = SequenceActivationLayout {
            actor: start.actor,
            x: start.startx,
            y: starty,
            width: self.width,
            height: (stop_y - starty).max(0.0),
            depth: stack.len(),
        };
        self.groups[group_index] = Some((layout, actor_index));
        self.rect_count += 1;
        self.groups[group_index].as_ref().map(|(layout, _)| layout)
    }

    pub fn include_inserted_top(
        &mut self,
        top_y: f64,
        box_margin: f64,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        // Overlapping activations can amplify the otherwise prepaid linear event traversal.
        // Admit this exact scan before mutating its envelopes.
        work_meter.charge(self.active.len())?;
        // Mermaid visits activations after all sequence items in updateBounds. Their margin
        // multipliers are therefore 0, -1, -2, ... in activation creation order.
        for (index, start) in self.active.values_mut().enumerate() {
            checkpoint_layout_iteration(work_meter, index)?;
            start.starty = start.starty.min(top_y + index as f64 * box_margin);
        }
        Ok(())
    }

    pub fn finish(
        self,
        work_meter: &OperationWorkMeter,
    ) -> Result<(
        Vec<SequenceActivationLayout>,
        SequencePreparedActivationGeometry,
    )> {
        let mut activations = Vec::with_capacity(self.rect_count);
        let mut groups = Vec::with_capacity(self.groups.len());
        for (index, group) in self.groups.into_iter().enumerate() {
            checkpoint_layout_iteration(work_meter, index)?;
            groups.push(group.map(|(layout, actor_index)| {
                let group = SequenceActivationGroup {
                    layout_index: activations.len(),
                    actor_index,
                };
                activations.push(layout);
                group
            }));
        }
        work_meter.checkpoint(OperationPhase::Layout)?;
        Ok((
            activations,
            SequencePreparedActivationGeometry {
                groups,
                group_by_start_id: self.group_by_start_id,
            },
        ))
    }
}

fn checkpoint_layout_iteration(work_meter: &OperationWorkMeter, index: usize) -> Result<()> {
    if index.is_multiple_of(SEQUENCE_CHECKPOINT_INTERVAL) {
        work_meter.checkpoint(OperationPhase::Layout)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use merman_render_core::{
        Error,
        resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId},
    };

    #[test]
    fn activation_envelope_scans_exhaust_work_before_mutating_the_next_event() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 2)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut builder = super::SequenceActivationGeometryBuilder::new(10.0);
        builder.start("first", "A", 0, 50.0, 100.0);
        builder.start("second", "A", 0, 50.0, 100.0);
        builder.include_inserted_top(90.0, 10.0, &meter).unwrap();
        assert_eq!(meter.used(), 2);
        let before: Vec<_> = builder.active.values().map(|start| start.starty).collect();
        let error = builder.include_inserted_top(0.0, 10.0, &meter).unwrap_err();
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected activation envelope work ceiling");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 4);
        assert_eq!(meter.used(), 2);
        assert_eq!(
            builder
                .active
                .values()
                .map(|start| start.starty)
                .collect::<Vec<_>>(),
            before
        );
    }

    #[test]
    fn prepared_activation_geometry_keeps_unmatched_groups() {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let mut builder = super::SequenceActivationGeometryBuilder::new(10.0);
        builder.start("open", "A", 0, 50.0, 100.0);
        builder.start("closed", "B", 1, 100.0, 100.0);
        assert!(builder.end(1, 150.0).is_some());
        let (layouts, geometry) = builder.finish(&meter).unwrap();
        assert_eq!(layouts.len(), 1);
        assert!(geometry.group_for_start("unknown").is_none());
        assert!(matches!(geometry.group_for_start("open"), Some(None)));
        let closed = geometry.group_for_start("closed").unwrap().unwrap();
        assert_eq!(closed.layout_index, 0);
        assert_eq!(closed.actor_index, 1);
    }
}
