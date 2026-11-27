use crate::resources::OperationWorkMeter;
use crate::route_paint::{Clearance, ObstacleOwner};
use merman_model::OperationPhase;
use std::sync::{Mutex, OnceLock, PoisonError};

/// One obstacle inventory and at most one tree for the shared axis routes.
#[derive(Debug, Default)]
pub struct AxisClearance {
    obstacles: Mutex<Option<Vec<(kurbo::Rect, ObstacleOwner)>>>,
    tree: OnceLock<Clearance>,
}

impl AxisClearance {
    /// The layout adapter has already admitted the cost of building the tree.
    pub fn new(obstacles: Vec<(kurbo::Rect, ObstacleOwner)>) -> Self {
        Self {
            obstacles: Mutex::new(Some(obstacles)),
            tree: OnceLock::new(),
        }
    }

    pub fn resolve(&self, work: &OperationWorkMeter) -> crate::Result<&Clearance> {
        self.resolve_with(work, Vec::new)
    }

    /// Builds a previously admitted obstacle inventory on first use.
    pub fn resolve_with(
        &self,
        work: &OperationWorkMeter,
        obstacles: impl FnOnce() -> Vec<(kurbo::Rect, ObstacleOwner)>,
    ) -> crate::Result<&Clearance> {
        if let Some(tree) = self.tree.get() {
            return Ok(tree);
        }
        work.checkpoint(OperationPhase::Layout)?;
        let tree = self.tree.get_or_init(|| {
            let prepared = self
                .obstacles
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            Clearance::new(prepared.unwrap_or_else(obstacles))
        });
        work.checkpoint(OperationPhase::Layout)?;
        Ok(tree)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;

    #[test]
    fn deferred_inventory_is_built_once_and_yields_to_prepared_geometry() {
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let deferred = AxisClearance::default();
        let first = deferred
            .resolve_with(&work, || {
                vec![(kurbo::Rect::new(0.0, 0.0, 4.0, 4.0), ObstacleOwner::Node(0))]
            })
            .unwrap();
        let next = deferred
            .resolve_with(&work, || panic!("inventory rebuilt"))
            .unwrap();
        assert!(std::ptr::eq(first, next));

        let prepared = AxisClearance::new(Vec::new());
        prepared
            .resolve_with(&work, || panic!("inventory was supplied by placement"))
            .unwrap();
    }
}
