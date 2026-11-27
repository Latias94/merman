//! State backend dispatch and shared adapter admission.

use super::StateDiagramModel;
use crate::Result;
use crate::graph_layout::RouteContract;
#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
use crate::layout_backend::GraphLayoutBackend;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::StateDiagramLayout;
use merman_render_graph::state::state_layout_adapter_work;
#[cfg(feature = "layout-shoal")]
pub(super) use merman_render_graph::state::{
    note_group_owner_id, state_hidden_prefixes, validate_state_parent_cycles,
};
use serde_json::Value;

#[cfg_attr(
    not(any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    )),
    allow(
        unused_variables,
        reason = "The State admission entry point also exists without a solver."
    )
)]
pub(crate) fn layout_state_diagram_typed_with_work_meter(
    model: &StateDiagramModel,
    effective_config: &Value,
    style_plan: &super::StateStylePlan,
    label_sidecar: Option<&super::StateLabelSidecarBuilder>,
    execution: &crate::LayoutExecution<'_>,
) -> Result<(StateDiagramLayout, RouteContract)> {
    let backend = execution.layout_backend();
    let mut work_control = OperationLayoutWorkControl::new(execution.work_meter());
    let adapter_work = state_layout_adapter_work(model, &work_control)?;
    work_control.charge_adapter(adapter_work)?;
    backend.validate_rootless_graph()?;
    match backend {
        #[cfg(feature = "layout-dagre")]
        GraphLayoutBackend::Dagre => merman_render_graph::state::layout_dagre(
            model,
            effective_config,
            style_plan,
            execution.text_measurer(),
            label_sidecar,
            &mut work_control,
        )
        .map(|layout| (layout, RouteContract::DagreParity)),
        #[cfg(feature = "layout-elk")]
        GraphLayoutBackend::Elk(algorithm) => merman_render_graph::state::layout_elk(
            model,
            effective_config,
            algorithm.algorithm(),
            style_plan,
            label_sidecar,
            execution.text_measurer(),
            execution.elk_operation_seed(),
            &mut work_control,
        )
        .map(|layout| (layout, RouteContract::ElkParity)),
        #[cfg(feature = "layout-shoal")]
        GraphLayoutBackend::Shoal => super::shoal::layout_state_diagram_shoal(
            model,
            effective_config,
            style_plan,
            label_sidecar,
            execution,
            &mut work_control,
        )
        .map(|(layout, routes)| (layout, RouteContract::canonical_graph(routes))),
        _ => Err(backend.unavailable_for("stateDiagram")),
    }
}
