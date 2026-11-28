//! State backend dispatch and shared adapter admission.

use super::{StateDiagramModel, StateLabelSidecar, StateStylePlan};
use crate::Result;
use crate::graph_layout::RouteContract;
use crate::layout_backend::GraphLayoutBackend;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::StateDiagramLayout;
#[cfg(feature = "layout-layered")]
use merman_model::OperationPhase;
use merman_model::presentation::PresentationConfig;
use merman_render_graph::state::{StateLabelSidecarBuilder, state_layout_adapter_work};
#[cfg(feature = "layout-layered")]
use merman_render_graph::state::{StateLayeredError, StateLayeredPaint, layout_layered};
#[cfg(feature = "layout-shoal")]
pub(super) use merman_render_graph::state::{
    note_group_owner_id, state_hidden_prefixes, validate_state_parent_cycles,
};

type PreparedState = (
    StateDiagramLayout,
    RouteContract,
    StateLabelSidecar,
    StateStylePlan,
);

pub(crate) fn prepare_state_diagram(
    model: &StateDiagramModel,
    config: &PresentationConfig,
    execution: &crate::LayoutExecution<'_>,
) -> Result<PreparedState> {
    let backend = execution.layout_backend();
    #[cfg(feature = "layout-layered")]
    if backend == GraphLayoutBackend::Layered {
        return prepare_layered(model, config, execution);
    }
    prepare_classic(model, config, backend, execution)
}

#[cfg(feature = "layout-layered")]
fn prepare_layered(
    model: &StateDiagramModel,
    config: &PresentationConfig,
    execution: &crate::LayoutExecution<'_>,
) -> Result<PreparedState> {
    use std::sync::Arc;

    let work = execution.work_meter();
    let attempt = {
        let style = resolve_style(model, config, GraphLayoutBackend::Layered, execution);
        let labels = StateLabelSidecarBuilder::default();
        admit(
            model,
            GraphLayoutBackend::Layered,
            &mut OperationLayoutWorkControl::new(Arc::clone(&work)),
        )?;
        layout_layered(
            model,
            config.as_config(),
            &style,
            execution.text_measurer(),
            &labels,
            &work,
        )
        .map(|(layout, paint)| {
            (
                layout,
                RouteContract::StateLayered(Arc::new(paint)),
                labels.finish(),
                style,
            )
        })
    };
    work.checkpoint(OperationPhase::Layout)?;
    match attempt {
        Ok(prepared) => Ok(prepared),
        Err(StateLayeredError::Host(error)) => Err(error),
        Err(error) if error.is_recoverable() => {
            let dagre = GraphLayoutBackend::Dagre;
            if !cfg!(feature = "layout-dagre")
                || !execution
                    .family
                    .session()
                    .supports_capability(crate::RenderCapability::LayoutDagre)
            {
                return Err(dagre.unavailable_for("stateDiagram"));
            }
            let (layout, _, labels, style) = prepare_classic(model, config, dagre, execution)?;
            work.checkpoint(OperationPhase::Layout)?;
            Ok((
                layout,
                RouteContract::StateLayered(Arc::new(StateLayeredPaint::dagre_fallback())),
                labels,
                style,
            ))
        }
        Err(error) => Err(crate::Error::InvalidModel {
            message: format!("State Layered layout failed: {error}"),
        }),
    }
}

fn resolve_style(
    model: &StateDiagramModel,
    config: &PresentationConfig,
    backend: GraphLayoutBackend,
    execution: &crate::LayoutExecution<'_>,
) -> StateStylePlan {
    StateStylePlan::resolve(
        model,
        config,
        backend == GraphLayoutBackend::Dagre,
        execution.native_family_theme(),
        execution.family.session().canvas_background(),
    )
}

fn prepare_classic(
    model: &StateDiagramModel,
    effective_config: &PresentationConfig,
    backend: GraphLayoutBackend,
    execution: &crate::LayoutExecution<'_>,
) -> Result<PreparedState> {
    let label_sidecar = StateLabelSidecarBuilder::default();
    let style = resolve_style(model, effective_config, backend, execution);
    let (layout, routes) = layout_classic(
        model,
        effective_config,
        &style,
        &label_sidecar,
        backend,
        execution,
    )?;
    Ok((layout, routes, label_sidecar.finish(), style))
}

fn admit(
    model: &StateDiagramModel,
    backend: GraphLayoutBackend,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<()> {
    let adapter_work = state_layout_adapter_work(model, work_control)?;
    work_control.charge_adapter(adapter_work)?;
    backend.validate_rootless_graph()
}

#[cfg_attr(
    not(any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-grid",
        feature = "layout-shoal"
    )),
    allow(
        unused_variables,
        reason = "The State admission entry point also exists without a solver."
    )
)]
fn layout_classic(
    model: &StateDiagramModel,
    effective_config: &PresentationConfig,
    style_plan: &StateStylePlan,
    label_sidecar: &StateLabelSidecarBuilder,
    backend: GraphLayoutBackend,
    execution: &crate::LayoutExecution<'_>,
) -> Result<(StateDiagramLayout, RouteContract)> {
    let mut work_control = OperationLayoutWorkControl::new(execution.work_meter());
    admit(model, backend, &mut work_control)?;
    match backend {
        #[cfg(feature = "layout-grid")]
        GraphLayoutBackend::Grid => merman_render_graph::state::layout_grid(
            model,
            effective_config,
            style_plan,
            execution.text_measurer(),
            label_sidecar,
            &execution.work_meter(),
        )
        .map(|(layout, paint)| (layout, RouteContract::Grid(paint))),
        #[cfg(feature = "layout-dagre")]
        GraphLayoutBackend::Dagre => merman_render_graph::state::layout_dagre(
            model,
            effective_config.as_value(),
            style_plan,
            execution.text_measurer(),
            Some(label_sidecar),
            &mut work_control,
        )
        .map(|layout| (layout, RouteContract::DagreParity)),
        #[cfg(feature = "layout-elk")]
        GraphLayoutBackend::Elk(algorithm) => merman_render_graph::state::layout_elk(
            model,
            effective_config.as_value(),
            algorithm.algorithm(),
            style_plan,
            Some(label_sidecar),
            execution.text_measurer(),
            execution.elk_operation_seed(),
            &mut work_control,
        )
        .map(|layout| (layout, RouteContract::ElkParity)),
        #[cfg(feature = "layout-shoal")]
        GraphLayoutBackend::Shoal => super::shoal::layout_state_diagram_shoal(
            model,
            effective_config.as_value(),
            style_plan,
            Some(label_sidecar),
            execution,
            &mut work_control,
        )
        .map(|(layout, routes)| (layout, RouteContract::canonical_graph(routes))),
        _ => Err(backend.unavailable_for("stateDiagram")),
    }
}
