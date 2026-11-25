use super::charset::GraphCharset;
#[cfg(test)]
use super::layout::layout_graph_with_feedback;
use super::layout::{GraphLayout, LayoutFeedback, LayoutInput, MeasuredLayout};
use super::model::AsciiGraph;
use super::routing::{self, RouteScenePlan};
use crate::error::{AsciiError, Result};
use crate::operation::{AsciiExecution, GraphPlanningBudget};
use crate::options::GraphLayoutPolicy;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

#[cfg(test)]
mod tests;

pub(super) struct PlannedGraph<'graph> {
    pub(super) layout: GraphLayout,
    pub(super) routes: RouteScenePlan<'graph>,
}

struct InitialGraph<'graph> {
    measured: MeasuredLayout,
    feedback: LayoutFeedback,
    input: routing::SceneRoutingInput<'graph>,
    incumbent: Option<RouteScenePlan<'graph>>,
}

pub(super) fn plan_graph<'graph>(
    graph: &'graph AsciiGraph,
    policy: &GraphLayoutPolicy,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<PlannedGraph<'graph>> {
    let local_budget = GraphPlanningBudget::default();
    let execution = execution.with_graph_planning(&local_budget);
    execution.checkpoint(OperationPhase::Layout)?;
    let layout_input = LayoutInput::try_new(graph, policy, resources, execution)?;
    let InitialGraph {
        mut measured,
        mut feedback,
        input,
        incumbent,
    } = initial_layout(graph, &layout_input, charset, policy, resources, execution)?;
    if let Some(routes) = incumbent {
        return Ok(PlannedGraph {
            layout: measured.layout,
            routes,
        });
    }
    let mut next_feedback = 0;
    loop {
        execution.checkpoint(OperationPhase::Layout)?;
        let miss = match routing::route_scene_with_input(
            graph,
            &measured.layout,
            &input,
            charset,
            policy,
            resources,
            execution,
        ) {
            Ok(routes) => {
                return Ok(PlannedGraph {
                    layout: measured.layout,
                    routes,
                });
            }
            Err(
                miss @ (AsciiError::RoutingSearchExhausted { .. }
                | AsciiError::PortCapacityExceeded { .. }),
            ) if policy.expands_routing_space() => miss,
            Err(error) => return Err(error),
        };
        let allowance = &execution
            .graph_planning()
            .expect("scene planning budget")
            .reflow_work;
        let mut changed = false;
        while !changed && next_feedback < 2 && allowance.get() != 0 {
            let before = resources.layout_work_used();
            let bounded = resources
                .clone()
                .with_planning_work_allowance(allowance.get());
            let request = if next_feedback == 0 {
                feedback.record_failure_with_input(&layout_input, &measured, &miss, &bounded)
            } else {
                feedback.record_return_failure_with_input(&layout_input, &measured, &miss, &bounded)
            };
            next_feedback += 1;
            allowance.set(
                allowance
                    .get()
                    .saturating_sub(resources.layout_work_used().saturating_sub(before)),
            );
            changed = match request {
                Err(AsciiError::PlanningWorkAllowanceExhausted) => {
                    allowance.set(0);
                    return Err(miss);
                }
                result => result?,
            };
        }
        if !changed || allowance.get() == 0 {
            return Err(miss);
        }
        let before = resources.layout_work_used();
        drop(measured);
        let mut bounded = resources
            .clone()
            .with_planning_work_allowance(allowance.get());
        let next = layout_input.generate(&feedback, &mut bounded, execution);
        allowance.set(
            allowance
                .get()
                .saturating_sub(resources.layout_work_used().saturating_sub(before)),
        );
        measured = match next {
            Err(AsciiError::PlanningWorkAllowanceExhausted) => {
                allowance.set(0);
                return Err(miss);
            }
            result => result?,
        };
    }
}

fn initial_layout<'graph>(
    graph: &'graph AsciiGraph,
    input: &LayoutInput<'_>,
    charset: &GraphCharset,
    policy: &GraphLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<InitialGraph<'graph>> {
    let initial = input.generate(&LayoutFeedback::default(), resources, execution)?;
    let routes = routing::SceneRoutingInput::try_new(graph, &graph.edges, charset, resources)?;
    if !policy.expands_routing_space() {
        return Ok(InitialGraph {
            measured: initial,
            feedback: LayoutFeedback::default(),
            input: routes,
            incumbent: None,
        });
    }
    let allowance = &execution
        .graph_planning()
        .expect("scene planning budget")
        .reflow_work;
    let before = resources.layout_work_used();
    let mut bounded = resources
        .clone()
        .with_planning_work_allowance(allowance.get());
    let mut incumbent = None;
    let candidate = (|| {
        let mut feedback = LayoutFeedback::default();
        if !feedback.record_initial_returns_with_input(input, &initial, &bounded)? {
            return Ok(None);
        }
        // Spend at most half of the remaining preparation allowance proving that the
        // current geometry needs no growth. Keep the other half for a measured rebuild.
        let remaining = allowance
            .get()
            .saturating_sub(resources.layout_work_used() - before);
        let mut probe = bounded.clone().with_planning_work_allowance(remaining / 2);
        match routing::route_scene_with_input(
            graph,
            &initial.layout,
            &routes,
            charset,
            policy,
            &mut probe,
            execution,
        ) {
            Ok(routes) => {
                incumbent = Some(routes);
                return Ok(None);
            }
            Err(
                AsciiError::RoutingSearchExhausted { .. }
                | AsciiError::PortCapacityExceeded { .. }
                | AsciiError::PlanningWorkAllowanceExhausted,
            ) => {}
            Err(error) => return Err(error),
        }
        let measured = input.generate(&feedback, &mut bounded, execution)?;
        Ok(Some((measured, feedback)))
    })();
    allowance.set(
        allowance
            .get()
            .saturating_sub(resources.layout_work_used().saturating_sub(before)),
    );
    let (measured, feedback) = match candidate {
        Ok(Some(candidate)) => candidate,
        Ok(None) => (initial, LayoutFeedback::default()),
        Err(AsciiError::PlanningWorkAllowanceExhausted) => {
            allowance.set(0);
            (initial, LayoutFeedback::default())
        }
        Err(error) => return Err(error),
    };
    Ok(InitialGraph {
        measured,
        feedback,
        input: routes,
        incumbent,
    })
}
