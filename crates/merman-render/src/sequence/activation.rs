use merman_core::diagrams::sequence::SequenceMessage;
use std::collections::BTreeMap;

pub(super) struct SequenceActivationState<'a> {
    width: f64,
    stacks: BTreeMap<&'a str, Vec<f64>>,
}

impl<'a> SequenceActivationState<'a> {
    pub(super) fn new(width: f64) -> Self {
        Self {
            width,
            stacks: BTreeMap::new(),
        }
    }

    pub(super) fn handle_directive(
        &mut self,
        msg: &'a SequenceMessage,
        actor_index: &std::collections::HashMap<&'a str, usize>,
        actor_centers_x: &[f64],
    ) -> bool {
        match msg.message_type {
            // ACTIVE_START
            17 => {
                let Some(actor_id) = msg.from.as_deref() else {
                    return true;
                };
                let Some(&idx) = actor_index.get(actor_id) else {
                    return true;
                };
                let cx = actor_centers_x[idx];
                let stack = self.stacks.entry(actor_id).or_default();
                let stacked_size = stack.len();
                let startx = cx + (((stacked_size as f64) - 1.0) * self.width) / 2.0;
                stack.push(startx);
                true
            }
            // ACTIVE_END
            18 => {
                let Some(actor_id) = msg.from.as_deref() else {
                    return true;
                };
                if let Some(stack) = self.stacks.get_mut(actor_id) {
                    let _ = stack.pop();
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn actor_bounds(&self, actor_id: &str, center_x: f64) -> (f64, f64) {
        self.stacks
            .get(actor_id)
            .and_then(|s| s.last().copied())
            .map(|startx| (startx, startx + self.width))
            .unwrap_or((center_x - 1.0, center_x + 1.0))
    }

    pub(super) fn width(&self) -> f64 {
        self.width
    }
}
