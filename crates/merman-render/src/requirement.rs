//! Renderer-side backend admission and Requirement provider bridges.

use crate::layout_backend::GraphLayoutBackend;
#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
use crate::layout_work::OperationLayoutWorkControl;
#[cfg(test)]
use crate::resources::RenderResourcePolicy;
use crate::resources::{ModelComplexity, OperationWorkMeter};
use crate::{Result, text::TextMeasurer};
use merman_model::families::requirement::RequirementDiagramModel;
use merman_render_graph::requirement as graph;
#[cfg(test)]
use serde_json::Value;
#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
use std::sync::Arc;

pub(crate) use graph::{
    RequirementConfigView, RequirementNodeTypography, RequirementPaintThemePlan,
    element_node_label_sources, requirement_node_label_sources, sanitize_label_with_sink,
};

#[cfg(feature = "layout-elk")]
mod elk;
#[cfg(all(test, feature = "layout-dagre"))]
mod native_theme;
#[cfg(feature = "layout-shoal")]
mod shoal;

#[derive(Debug)]
pub(crate) struct RequirementPreparedArtifact {
    pub(crate) prepared: graph::RequirementPreparedArtifact,
    #[cfg(feature = "layout-shoal")]
    route_contract: crate::graph_layout::RouteContract,
}

impl RequirementPreparedArtifact {
    pub(crate) fn layout(&self) -> &crate::model::RequirementDiagramLayout {
        self.prepared.layout()
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn canonical_routes(
        &self,
    ) -> Option<&crate::graph_layout::canonical::CanonicalRoutes> {
        self.route_contract.canonical_routes()
    }
}

/// Lays out a Requirement model under the resource policy owned by the render operation.
#[cfg(test)]
pub(crate) fn layout_requirement_diagram_typed_with_resource_policy(
    model: &RequirementDiagramModel,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
    resource_limits: RenderResourcePolicy,
) -> Result<RequirementPreparedArtifact> {
    let work_meter = std::sync::Arc::new(OperationWorkMeter::new(resource_limits));
    let binding = graph::RequirementCssBinding::resolve(effective_config, None, None);
    let config = merman_model::MermaidConfig::from_value(effective_config.clone());
    layout_requirement_diagram_typed_with_work_meter_and_typography(
        model,
        &config,
        crate::layout_backend::resolve(
            crate::DiagramFamilyId::REQUIREMENT,
            "requirement",
            effective_config,
        )?,
        text_measurer,
        &binding,
        &work_meter,
        #[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
        None,
    )
}

/// Lays out a Requirement model with renderer-owned inherited typography.
#[cfg_attr(
    not(any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    )),
    allow(
        unused_variables,
        reason = "The no-engine entry point rejects layout before measurement."
    )
)]
pub(crate) fn layout_requirement_diagram_typed_with_work_meter_and_typography(
    model: &RequirementDiagramModel,
    config: &merman_model::MermaidConfig,
    backend: GraphLayoutBackend,
    text_measurer: &dyn TextMeasurer,
    css_binding: &graph::RequirementCssBinding,
    work_meter: &std::sync::Arc<OperationWorkMeter>,
    #[cfg(any(feature = "layout-elk", feature = "layout-shoal"))] execution: Option<
        &crate::LayoutExecution<'_>,
    >,
) -> Result<RequirementPreparedArtifact> {
    work_meter
        .policy()
        .check_model_complexity(ModelComplexity::from_requirement(model))?;
    if backend == GraphLayoutBackend::Shoal {
        #[cfg(feature = "layout-shoal")]
        return shoal::layout(
            model,
            config,
            text_measurer,
            css_binding,
            work_meter,
            execution,
        );
        #[cfg(not(feature = "layout-shoal"))]
        return Err(backend.unavailable_for("requirement"));
    }
    work_meter.charge(graph::requirement_layout_work_units(model))?;
    backend.validate_rootless_graph()?;
    #[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
    {
        let mut work_control = OperationLayoutWorkControl::new(Arc::clone(work_meter));
        let settings = &css_binding.layout;
        let measured =
            graph::measure_requirement(model, config, text_measurer, settings, work_meter)?;
        let prepared = match backend {
            #[cfg(feature = "layout-elk")]
            GraphLayoutBackend::Elk(algorithm) => elk::layout(
                &model.relationships,
                measured,
                config.as_value(),
                text_measurer,
                algorithm,
                execution.map_or_else(
                    || {
                        merman_layout_elk::ElkOperationSeed::from_operation_seed(
                            std::num::NonZeroU64::MIN,
                        )
                    },
                    crate::LayoutExecution::elk_operation_seed,
                ),
                &mut work_control,
            ),
            #[cfg(feature = "layout-dagre")]
            GraphLayoutBackend::Dagre => graph::layout_dagre(
                &model.relationships,
                measured,
                settings,
                text_measurer,
                &mut work_control,
            ),
            _ => return Err(backend.unavailable_for("requirement")),
        }?;
        Ok(RequirementPreparedArtifact {
            prepared,
            #[cfg(feature = "layout-shoal")]
            route_contract: crate::graph_layout::RouteContract::default(),
        })
    }
    #[cfg(not(any(feature = "layout-dagre", feature = "layout-elk")))]
    Err(backend.unavailable_for("requirement"))
}
