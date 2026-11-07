use super::{SequenceLayoutCheckpoints, sequence_activation_start_x};
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct SequenceActivationStart {
    startx: f64,
    starty: f64,
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

#[derive(Debug, Default)]
pub(crate) struct SequencePreparedActivationGeometry {
    pub(crate) groups: Vec<Option<SequenceActivationRect>>,
    pub(crate) group_by_start_id: FxHashMap<String, usize>,
    rect_count: usize,
}
impl SequencePreparedActivationGeometry {
    pub(crate) const fn rect_count(&self) -> usize {
        self.rect_count
    }
}

pub(super) struct SequenceActivationGeometryBuilder {
    width: f64,
    stacks: BTreeMap<usize, Vec<usize>>,
    active: BTreeMap<usize, SequenceActivationStart>,
    geometry: SequencePreparedActivationGeometry,
}

impl SequenceActivationGeometryBuilder {
    pub(super) fn new(width: f64) -> Self {
        Self {
            width,
            stacks: BTreeMap::new(),
            active: BTreeMap::new(),
            geometry: SequencePreparedActivationGeometry::default(),
        }
    }

    pub(super) fn start(
        &mut self,
        message_id: &str,
        actor_index: usize,
        center_x: f64,
        cursor_y: f64,
    ) {
        let stack = self.stacks.entry(actor_index).or_default();
        let group_index = self.geometry.groups.len();
        self.active.insert(
            group_index,
            SequenceActivationStart {
                startx: sequence_activation_start_x(center_x, stack.len(), self.width),
                starty: cursor_y + 2.0,
            },
        );
        stack.push(group_index);
        self.geometry.groups.push(None);
        self.geometry
            .group_by_start_id
            .insert(message_id.to_string(), group_index);
    }

    pub(super) fn end(
        &mut self,
        actor_index: usize,
        cursor_y: f64,
    ) -> Option<&SequenceActivationRect> {
        let stack = self.stacks.get_mut(&actor_index)?;
        let group_index = stack.pop()?;
        let start = self.active.remove(&group_index)?;
        let mut starty = start.starty;
        let mut stop_y = cursor_y;
        if starty + 18.0 > stop_y {
            starty = stop_y - 6.0;
            stop_y += 12.0;
        }
        self.geometry.groups[group_index] = Some(SequenceActivationRect {
            startx: start.startx,
            starty,
            width: self.width,
            height: (stop_y - starty).max(0.0),
            class_idx: stack.len() % 3,
            actor_index,
        });
        self.geometry.rect_count += 1;
        self.geometry.groups[group_index].as_ref()
    }

    pub(super) fn include_inserted_top(
        &mut self,
        top_y: f64,
        box_margin: f64,
        checkpoints: SequenceLayoutCheckpoints<'_>,
    ) -> crate::Result<()> {
        // Overlapping activations can amplify the otherwise prepaid linear event traversal.
        // Admit this exact scan before mutating its envelopes.
        checkpoints.work_meter.charge(self.active.len())?;
        // Mermaid visits activations after all sequence items in updateBounds. Their margin
        // multipliers are therefore 0, -1, -2, ... in activation creation order.
        for (index, start) in self.active.values_mut().enumerate() {
            checkpoints.checkpoint_loop(index)?;
            start.starty = start.starty.min(top_y + index as f64 * box_margin);
        }
        Ok(())
    }

    pub(super) fn finish(self) -> SequencePreparedActivationGeometry {
        self.geometry
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use merman_model::families::sequence::SequenceMessageKind;

    #[test]
    fn activation_envelope_scans_exhaust_work_before_mutating_the_next_event() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, 2)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let checkpoints = super::SequenceLayoutCheckpoints::for_layout(&meter);
        let mut builder = super::SequenceActivationGeometryBuilder::new(10.0);
        builder.start("first", 0, 50.0, 100.0);
        builder.start("second", 0, 50.0, 100.0);
        builder
            .include_inserted_top(90.0, 10.0, checkpoints)
            .unwrap();
        assert_eq!(meter.used(), 2);
        let before: Vec<_> = builder.active.values().map(|start| start.starty).collect();
        let error = builder
            .include_inserted_top(0.0, 10.0, checkpoints)
            .unwrap_err();
        let crate::Error::ResourceLimitExceeded(error) = error else {
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
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nA->>+B: Hello\nB-->>-A: Reply",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let merman_model::presentation::RenderSemanticModel::Sequence(model) = parsed.model()
        else {
            panic!("expected Sequence model");
        };
        let work = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let prepared = super::super::prepare_sequence_diagram_typed_with_title_and_work_meter(
            model,
            None,
            &merman_model::presentation::PresentationConfig::default(),
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
        let unmatched = super::super::prepare_sequence_diagram_typed_with_title_and_work_meter(
            &unmatched,
            None,
            &merman_model::presentation::PresentationConfig::default(),
            None,
            &crate::text::DeterministicTextMeasurer::default(),
            None,
            work,
        )
        .unwrap();
        let geometry = unmatched.activation_geometry();
        assert_eq!(geometry.rect_count(), 0);
        assert_eq!(geometry.groups.len(), 1);
        assert!(geometry.groups[0].is_none());
        assert_eq!(geometry.group_by_start_id.len(), 1);
    }
}
