//! Connects shared Shoal geometry to the render operation's solve stage.

use super::ir::MeasuredGraph;
use crate::Result;
use crate::layout_work::OperationLayoutWorkControl;
use merman_layout_shoal as shoal;
pub(crate) use merman_render_core::graph_layout::shoal::{
    Constraints, NodeHint, OrderHint, Placement, WidthPolicy,
};
use merman_render_core::graph_layout::shoal::{project_input, project_output};

#[cfg(all(
    test,
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    feature = "layout-elk"
))]
use super::ir::{Endpoint, Marker, Size};
#[cfg(all(
    test,
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    feature = "layout-elk"
))]
#[path = "kernel_bench.rs"]
mod kernel_bench;

#[allow(
    dead_code,
    reason = "The shared layered placement solver calls this bridge."
)]
pub(crate) fn solve(
    measured: &MeasuredGraph,
    placement: Placement<'_>,
    split_incoming_ports: &[usize],
    scratch: &mut shoal::LayoutScratch,
    work: &mut OperationLayoutWorkControl,
) -> Result<super::ir::SolvedGraph> {
    let options = placement.options_for(measured);
    #[cfg(all(
        test,
        feature = "diagram-flowchart",
        feature = "layout-dagre",
        feature = "layout-elk"
    ))]
    kernel_bench::capture(measured, options);
    let mut graph = project_input(measured, &placement, work)?;
    if !split_incoming_ports.is_empty() {
        work.charge_adapter(split_incoming_ports.len())?;
        for &index in split_incoming_ports {
            work.charge_adapter(0)?;
            let node = graph
                .nodes
                .get_mut(index)
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: "Shoal split port hint does not match a measured node occurrence"
                        .into(),
                })?;
            node.split_incoming_ports = true;
        }
    }
    let solved = crate::layout_work::measure_layout_solve!(shoal::layout_geometry_with_control(
        &graph, &options, scratch, work
    ))
    .map_err(|error| work.map_shoal_error(error))?;
    project_output(measured, solved, work)
}
