//! Family-owned Usecase projection into the shared spatial graph renderer.

mod adapter;
mod json_table;
mod validation;

#[cfg(test)]
mod tests;

use crate::error::Result;
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, GraphLayoutPolicy};
use merman_model::OperationPhase;
use merman_model::families::usecase::UsecaseDiagramModel;

pub(crate) fn render_usecase_diagram_with_execution(
    model: &UsecaseDiagramModel,
    options: &AsciiRenderOptions,
    policy: GraphLayoutPolicy,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(OperationPhase::Semantic)?;
    let resources = execution.new_resource_context(OperationPhase::Semantic);
    resources.transaction(|resources| {
        let result = resources.transaction_preserving_layout_work(|resources| {
            let graph =
                adapter::project(model, options.terminal_width_profile, resources, execution)?;
            execution.checkpoint(OperationPhase::Layout)?;
            let mut layout_resources =
                execution.resource_context(resources, OperationPhase::Layout);
            crate::graph::render_graph_with_resolved_policy_and_execution(
                &graph,
                options,
                policy,
                &mut layout_resources,
                execution,
            )
        });
        match result {
            Err(error)
                if execution.preserves_graph_work_on_error(&error, options.graph_presentation) =>
            {
                Ok(Err(error))
            }
            Ok(rendered) => Ok(Ok(rendered)),
            Err(error) => Err(error),
        }
    })?
}
