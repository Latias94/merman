//! Selects a Usecase backend and pairs prepared graph data with its terminal routes.

use crate::{LayoutExecution, Result, elk_edge_geometry};
use merman_model::{families::usecase::UsecaseDiagramModel, presentation::PresentationConfig};
use merman_render_graph::usecase::{UsecasePrepared, prepare_usecase_measurements};

pub use merman_render_graph::usecase::UsecaseDiagramLayout;
#[cfg(feature = "layout-shoal")]
use merman_render_graph::usecase::UsecaseLabelPlan;
#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_graph::usecase::actor;
pub(crate) use merman_render_graph::usecase::{UsecaseEdgePlan, UsecaseNodePlan, requires_math};

mod layout;
#[cfg(feature = "layout-shoal")]
mod shoal;
#[cfg(feature = "layout-shoal")]
pub(crate) use shoal::{UsecaseMarker, canonical_route_style};

#[derive(Debug)]
pub(crate) struct UsecasePreparedArtifact {
    pub(crate) prepared: UsecasePrepared,
    #[cfg(feature = "layout-shoal")]
    route_contract: crate::graph_layout::RouteContract,
    #[cfg(feature = "layout-shoal")]
    canonical: Option<shoal::UsecaseCanonicalLayout>,
}

impl UsecasePreparedArtifact {
    pub(crate) fn layout(&self) -> &UsecaseDiagramLayout {
        self.prepared.layout()
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn canonical_routes(
        &self,
    ) -> Option<&crate::graph_layout::canonical::CanonicalRoutes> {
        self.route_contract.canonical_routes()
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn canonical_boundary_geometry(
        &self,
    ) -> Option<(
        &std::collections::HashMap<String, crate::model::LayoutLabel>,
        &[(usize, usize)],
    )> {
        self.canonical
            .as_ref()
            .map(|canonical| (&canonical.titles, canonical.boundary_order.as_slice()))
    }
}

#[cfg(any(feature = "layout-shoal", feature = "layout-elk"))]
mod measure {
    pub(super) use merman_render_graph::usecase::configured_usecase_number as number;
}

pub(crate) fn prepare_usecase_diagram(
    model: &UsecaseDiagramModel,
    effective_config: &PresentationConfig,
    execution: &LayoutExecution<'_>,
) -> Result<UsecasePreparedArtifact> {
    #[cfg(any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    ))]
    use crate::graph_layout::RouteContract;
    use crate::layout_backend::GraphLayoutBackend;

    let work_meter = execution.work_meter();
    work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
    let config_value = effective_config.as_value();
    let mut work = crate::layout_work::OperationLayoutWorkControl::new(work_meter);
    let measured = prepare_usecase_measurements(
        model,
        effective_config,
        execution.native_family_theme(),
        execution.family.session().canvas_background(),
        execution.text_measurer(),
        execution.math_renderer(),
        &mut work,
    )?;
    let backend = execution.layout_backend();
    backend.validate_rootless_graph()?;
    #[cfg(feature = "layout-shoal")]
    if backend == GraphLayoutBackend::Shoal {
        let (layout, canonical, routes) = shoal::layout(
            model,
            config_value,
            &measured.nodes,
            &measured.edges,
            Some(execution),
            &mut work,
        )?;
        return Ok(UsecasePreparedArtifact {
            prepared: UsecasePrepared::new(measured, layout, Default::default(), false),
            route_contract: RouteContract::canonical_graph(routes),
            canonical: Some(canonical),
        });
    }
    #[cfg(feature = "layout-dagre")]
    let mut measured = measured;
    let (mut layout, route_contract) = match backend {
        #[cfg(feature = "layout-dagre")]
        GraphLayoutBackend::Dagre => Ok((
            merman_render_graph::usecase::layout_dagre(
                model,
                config_value,
                &mut measured,
                &mut work,
            )?,
            RouteContract::DagreParity,
        )),
        #[cfg(feature = "layout-elk")]
        GraphLayoutBackend::Elk(algorithm) => Ok((
            layout::elk_layout(
                model,
                config_value,
                &measured.nodes,
                &measured.edges,
                &mut work,
                algorithm,
                execution.elk_operation_seed(),
            )?,
            RouteContract::ElkParity,
        )),
        _ => Err(backend.unavailable_for(execution.family_id().as_str())),
    }?;
    let edge_paths = layout::prepare_edge_paths(
        &mut layout,
        &measured.nodes,
        &measured.edges,
        config_value,
        &route_contract,
        &mut work,
    )?;
    let elk = route_contract.is_elk_parity();
    Ok(UsecasePreparedArtifact {
        prepared: UsecasePrepared::new(measured, layout, edge_paths, elk),
        #[cfg(feature = "layout-shoal")]
        route_contract,
        #[cfg(feature = "layout-shoal")]
        canonical: None,
    })
}

#[cfg(all(test, feature = "layout-dagre"))]
mod tests;
