//! Boundary-local card placement and straight-first orthogonal routing for C4 diagrams.

mod place;
#[cfg(test)]
mod tests;

use crate::grid::{Hierarchy, Metrics, Projection, rect, route};
use crate::validate;
use crate::work::{Work, reserved};
use crate::{
    Graph, LayoutError, LayoutGeometry, LayoutOutput, Options, PortSideHint, Rect, Side,
    WorkControl,
};

/// Optional column limits for cards and child boundaries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CardGridOptions {
    pub card_columns: Option<usize>,
    pub boundary_columns: Option<usize>,
    preferred: [bool; 2],
}

impl CardGridOptions {
    pub fn with_column_limits(
        card_columns: Option<usize>,
        boundary_columns: Option<usize>,
    ) -> Self {
        Self {
            card_columns,
            boundary_columns,
            ..Self::default()
        }
    }

    /// Treats selected counts as row targets, subject to the available width.
    /// In mixed scopes the two kinds then occupy separate grids, with cards first.
    pub fn prefer_columns(mut self, cards: bool, boundaries: bool) -> Self {
        self.preferred = [
            cards && self.card_columns.is_some(),
            boundaries && self.boundary_columns.is_some(),
        ];
        self
    }

    fn has_preferred_columns(self) -> bool {
        self.preferred.contains(&true)
    }
}

/// Places each boundary's cards on a compact grid in authored order. Nested boundaries occupy
/// their own cells, and a requested width can reduce the number of columns at every depth.
/// Geometry remains in input order. Captured ranks are local row indices.
pub fn layout_with_control(
    graph: &Graph,
    options: &Options,
    control: &mut impl WorkControl,
) -> Result<LayoutGeometry, LayoutError> {
    layout_with_hints(graph, options, CardGridOptions::default(), control)
}

/// Places a card grid with caller-supplied row limits. `None` uses the aspect target.
pub fn layout_with_hints(
    graph: &Graph,
    options: &Options,
    hints: CardGridOptions,
    control: &mut impl WorkControl,
) -> Result<LayoutGeometry, LayoutError> {
    let mut work = Work::new(control);
    work.check()?;
    validate::options(options)?;
    if !graph.lane_order.is_empty() {
        return Err(LayoutError::Unsupported("swimlanes on a card grid"));
    }
    let mut hierarchy = Hierarchy::new(graph, &mut work)?;
    if hints.has_preferred_columns() {
        place::group_items(&mut hierarchy, &mut work)?;
    }
    let mut metrics = Metrics::new(options);
    let projections = hierarchy.edges(graph, &mut metrics, &mut work)?;
    let caption_demands =
        place::caption_demands(graph, &hierarchy, &projections, metrics, &mut work)?;
    let (mut drawing, gutters) = place::place(
        graph,
        &hierarchy,
        &caption_demands,
        metrics,
        options,
        hints,
        &mut work,
    )?;

    work.charge(projections.len())?;
    let mut edges = reserved(projections.len())?;
    for (edge, projection) in graph.edges.iter().zip(projections) {
        work.visit(1)?;
        let mut sides = facing(projection, &drawing);
        for (side, hint) in sides.iter_mut().zip([edge.source_port, edge.target_port]) {
            if let PortSideHint::Side(fixed) | PortSideHint::FixedOffset { side: fixed, .. } = hint
            {
                *side = fixed;
            }
        }
        edges.push((projection, sides));
    }
    route::draw(
        graph,
        &hierarchy,
        &edges,
        metrics,
        options,
        &mut drawing,
        route::Gutters::CardGrid(&gutters),
        &mut work,
    )?;
    work.finish()?;
    Ok(drawing.into())
}

fn facing(projection: Projection, drawing: &LayoutOutput) -> [Side; 2] {
    let [source, target] = projection.items;
    match (source, target) {
        (Some(a), Some(b)) if a != b => {
            let (a, b) = (rect(drawing, a), rect(drawing, b));
            let (source, target) = (a.center(), b.center());
            let horizontal = if source.x <= target.x {
                [Side::Right, Side::Left]
            } else {
                [Side::Left, Side::Right]
            };
            let vertical = if source.y <= target.y {
                [Side::Bottom, Side::Top]
            } else {
                [Side::Top, Side::Bottom]
            };
            let x_overlap = a.x < b.x + b.width && b.x < a.x + a.width;
            let y_overlap = a.y < b.y + b.height && b.y < a.y + a.height;
            match (x_overlap, y_overlap) {
                (true, false) => vertical,
                (false, true) => horizontal,
                // Diagonal relations leave toward the open row or column beside the target.
                _ if source.y < target.y => [vertical[0], horizontal[1]],
                _ => [horizontal[0], vertical[1]],
            }
        }
        (None, Some(child)) => {
            [inner_face(rect(drawing, projection.ends[0]), rect(drawing, child)); 2]
        }
        (Some(child), None) => {
            [inner_face(rect(drawing, projection.ends[1]), rect(drawing, child)); 2]
        }
        _ => [Side::Right; 2],
    }
}

/// Connecting to a containing boundary uses an inward lead on the nearest physical face.
fn inner_face(boundary: Rect, child: Rect) -> Side {
    [
        (child.x - boundary.x, Side::Left),
        (
            boundary.y + boundary.height - child.y - child.height,
            Side::Bottom,
        ),
        (
            boundary.x + boundary.width - child.x - child.width,
            Side::Right,
        ),
        (child.y - boundary.y, Side::Top),
    ]
    .into_iter()
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .expect("four boundary faces")
    .1
}
