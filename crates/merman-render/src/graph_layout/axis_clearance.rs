use super::route_paint::{Clearance, ObstacleOwner};
use crate::resources::OperationWorkMeter;
use merman_model::OperationPhase;
use std::sync::{Mutex, OnceLock, PoisonError};

/// One obstacle inventory and at most one tree for the shared axis routes.
#[derive(Debug, Default)]
pub(crate) struct AxisClearance {
    obstacles: Mutex<Vec<(kurbo::Rect, ObstacleOwner)>>,
    tree: OnceLock<Clearance>,
}

impl AxisClearance {
    /// The layout adapter has already admitted the cost of building the tree.
    pub(crate) fn new(obstacles: Vec<(kurbo::Rect, ObstacleOwner)>) -> Self {
        Self {
            obstacles: Mutex::new(obstacles),
            tree: OnceLock::new(),
        }
    }

    pub(crate) fn resolve(&self, work: &OperationWorkMeter) -> crate::Result<&Clearance> {
        if let Some(tree) = self.tree.get() {
            return Ok(tree);
        }
        work.checkpoint(OperationPhase::Layout)?;
        let tree = self.tree.get_or_init(|| {
            let obstacles = std::mem::take(
                &mut *self
                    .obstacles
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner),
            );
            Clearance::new(obstacles)
        });
        work.checkpoint(OperationPhase::Layout)?;
        Ok(tree)
    }
}
