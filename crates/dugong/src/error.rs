//! Public failures from the complete Dagre layout pipeline.

use crate::WorkError;
use crate::graphlib::EdgeKey;

/// Failure returned by the complete Dagre layout pipeline.
///
/// Low-level rank helpers and the complete pipeline preserve `minlen = 0`, matching Mermaid's
/// dagre-d3-es companion. Rankers that leave an edge endpoint and its route point coincident fail
/// transactionally with a typed error instead of reproducing the companion's JavaScript throw.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayoutError {
    DegenerateEdgeGeometry { edge: EdgeKey, node: String },
    Work(WorkError),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DegenerateEdgeGeometry { edge, node } => {
                write!(
                    f,
                    "cannot intersect edge {} -> {} with the rectangle centered at node {node}",
                    edge.v, edge.w,
                )?;
                if let Some(name) = edge.name.as_deref() {
                    write!(f, " ({name})")?;
                }
                Ok(())
            }
            Self::Work(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for LayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Work(error) => Some(error),
            Self::DegenerateEdgeGeometry { .. } => None,
        }
    }
}

impl From<WorkError> for LayoutError {
    fn from(error: WorkError) -> Self {
        Self::Work(error)
    }
}
