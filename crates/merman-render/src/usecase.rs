//! Selects a Usecase backend and pairs prepared graph data with its terminal routes.

use crate::{LayoutExecution, Result};
use merman_model::{families::usecase::UsecaseDiagramModel, presentation::PresentationConfig};
use merman_render_graph::usecase::{UsecasePrepared, prepare_usecase_measurements};

pub use merman_render_graph::usecase::UsecaseDiagramLayout;
#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_graph::usecase::actor;
pub(crate) use merman_render_graph::usecase::requires_math;
#[cfg(feature = "layout-shoal")]
use merman_render_graph::usecase::{UsecaseCanonicalSvg, UsecaseEdgePainter};

#[cfg(feature = "layout-shoal")]
mod shoal;

#[derive(Debug)]
pub(crate) struct UsecasePreparedArtifact {
    pub(crate) prepared: UsecasePrepared,
    #[cfg(feature = "layout-shoal")]
    route_contract: Option<crate::graph_layout::RouteContract>,
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
        self.route_contract.as_ref()?.canonical_routes()
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn canonical_svg<'a>(
        &'a self,
        paint_edges: &'a UsecaseEdgePainter<'a>,
    ) -> Option<UsecaseCanonicalSvg<'a>> {
        self.canonical
            .as_ref()
            .map(|canonical| UsecaseCanonicalSvg {
                titles: &canonical.titles,
                boundary_order: &canonical.boundary_order,
                panel_captions: &canonical.panel_captions,
                paint_edges,
            })
    }
}

pub(crate) fn prepare_usecase_diagram(
    model: &UsecaseDiagramModel,
    effective_config: &PresentationConfig,
    execution: &LayoutExecution<'_>,
) -> Result<UsecasePreparedArtifact> {
    #[cfg(feature = "layout-shoal")]
    use crate::graph_layout::RouteContract;
    #[cfg(any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-grid",
        feature = "layout-layered",
        feature = "layout-shoal"
    ))]
    use crate::layout_backend::GraphLayoutBackend;

    let work_meter = execution.work_meter();
    work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
    let backend = execution.layout_backend();
    #[cfg(feature = "layout-grid")]
    if backend == GraphLayoutBackend::Grid {
        return Ok(UsecasePreparedArtifact {
            prepared: merman_render_graph::usecase::prepare_grid(
                model,
                effective_config,
                execution.native_family_theme(),
                execution.family.session().canvas_background(),
                execution.text_measurer(),
                execution.math_renderer(),
                &work_meter,
            )?,
            #[cfg(feature = "layout-shoal")]
            route_contract: None,
            #[cfg(feature = "layout-shoal")]
            canonical: None,
        });
    }
    #[cfg(feature = "layout-layered")]
    let dagre_retry = if backend == GraphLayoutBackend::Layered {
        use merman_render_graph::usecase::{UsecaseLayeredError, prepare_layered};

        match prepare_layered(
            model,
            effective_config,
            execution.native_family_theme(),
            execution.family.session().canvas_background(),
            execution.text_measurer(),
            execution.math_renderer(),
            &work_meter,
        ) {
            Ok(prepared) => {
                return Ok(UsecasePreparedArtifact {
                    prepared,
                    #[cfg(feature = "layout-shoal")]
                    route_contract: None,
                    #[cfg(feature = "layout-shoal")]
                    canonical: None,
                });
            }
            Err(UsecaseLayeredError::Host(error)) => return Err(error),
            Err(error) if error.is_recoverable() => {
                work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
                if !cfg!(feature = "layout-dagre")
                    || !execution
                        .family
                        .session()
                        .supports_capability(crate::RenderCapability::LayoutDagre)
                {
                    return Err(crate::Error::MissingCapability {
                        capability: crate::RenderCapability::LayoutDagre,
                        diagram_type: "usecase".to_owned(),
                    });
                }
                true
            }
            Err(error) => {
                return Err(crate::Error::InvalidModel {
                    message: error.to_string(),
                });
            }
        }
    } else {
        false
    };
    #[cfg(feature = "layout-layered")]
    let backend = if dagre_retry {
        GraphLayoutBackend::Dagre
    } else {
        backend
    };
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
    backend.validate_rootless_graph()?;
    #[cfg(feature = "layout-shoal")]
    if backend == GraphLayoutBackend::Shoal {
        let (layout, canonical, routes) =
            shoal::layout(model, config_value, &measured, Some(execution), &mut work)?;
        return Ok(UsecasePreparedArtifact {
            prepared: UsecasePrepared::new(measured, layout, Default::default(), false),
            route_contract: Some(RouteContract::canonical_graph(routes)),
            canonical: Some(canonical),
        });
    }
    match (backend, config_value, measured) {
        #[cfg(feature = "layout-dagre")]
        (GraphLayoutBackend::Dagre, config, measured) => {
            let prepared =
                merman_render_graph::usecase::prepare_dagre(model, config, measured, &mut work)?;
            #[cfg(feature = "layout-layered")]
            let prepared = {
                let mut prepared = prepared;
                if dagre_retry {
                    prepared.mark_layered_dagre_fallback();
                }
                prepared
            };
            Ok(UsecasePreparedArtifact {
                prepared,
                #[cfg(feature = "layout-shoal")]
                route_contract: Some(RouteContract::DagreParity),
                #[cfg(feature = "layout-shoal")]
                canonical: None,
            })
        }
        #[cfg(feature = "layout-elk")]
        (GraphLayoutBackend::Elk(algorithm), config, measured) => Ok(UsecasePreparedArtifact {
            prepared: merman_render_graph::usecase::prepare_elk(
                model,
                config,
                measured,
                &mut work,
                algorithm.algorithm(),
                execution.elk_operation_seed(),
            )?,
            #[cfg(feature = "layout-shoal")]
            route_contract: Some(RouteContract::ElkParity),
            #[cfg(feature = "layout-shoal")]
            canonical: None,
        }),
        (backend, _, _) => Err(backend.unavailable_for(execution.family_id().as_str())),
    }
}

#[cfg(all(test, feature = "layout-dagre"))]
mod tests;
