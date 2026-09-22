//! Flowchart route ownership around the shared ELK terminal-channel pass.

use super::ClippedEdgeRoute;
use crate::elk_terminal_jogs::TerminalRoute;
use crate::model::LayoutPoint;
use crate::resources::OperationWorkMeter;

impl TerminalRoute for ClippedEdgeRoute {
    fn points(&self) -> &[LayoutPoint] {
        &self.points
    }

    fn origin(&self) -> (f64, f64) {
        (self.origin_x, self.origin_y)
    }

    fn replace_terminal_points(
        &mut self,
        points: Vec<LayoutPoint>,
        source_changed: bool,
        target_changed: bool,
    ) {
        self.points = points;
        // Connector removal invalidates only its own rounded-corner adapter mask.
        if source_changed {
            self.elk_endpoint_adapters.source = false;
        }
        if target_changed {
            self.elk_endpoint_adapters.target = false;
        }
    }
}

pub(in crate::svg::parity::flowchart) fn straighten_edge_terminals(
    routes: &mut [ClippedEdgeRoute],
    work: &OperationWorkMeter,
) -> crate::Result<()> {
    crate::elk_terminal_jogs::straighten_edge_terminals(routes, |units| {
        work.charge(units).map_err(Into::into)
    })
}
