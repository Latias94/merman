use super::charset::GraphCharset;
use super::layout::{GraphLayout, layout_graph_with_resources_and_execution};
use super::model::AsciiGraph;
use super::routing::{self, RouteScenePlan};
use crate::error::Result;
use crate::operation::AsciiExecution;
use crate::options::GraphLayoutPolicy;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

pub(super) struct PlannedGraph<'graph> {
    pub(super) layout: GraphLayout,
    pub(super) routes: RouteScenePlan<'graph>,
}

pub(super) fn plan_graph<'graph>(
    graph: &'graph AsciiGraph,
    policy: &GraphLayoutPolicy,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<PlannedGraph<'graph>> {
    execution.checkpoint(OperationPhase::Layout)?;
    let layout = layout_graph_with_resources_and_execution(graph, policy, resources, execution)?;
    let routes = routing::prepare_route_scene_with_execution(
        graph,
        &layout,
        &graph.edges,
        charset,
        policy,
        resources,
        execution,
    )?;
    Ok(PlannedGraph { layout, routes })
}
