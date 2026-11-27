//! Supplies operation and canonical route data to the Usecase SVG provider.

use super::SvgExecution;
#[cfg(feature = "layout-shoal")]
use super::svg_path_bounds_from_d;
#[cfg(feature = "layout-shoal")]
use crate::{Error, model::Bounds};
use crate::{Result, usecase::UsecasePreparedArtifact};
use merman_model::{
    families::usecase::UsecaseDiagramModel, family::DiagramFamilyId,
    presentation::PresentationConfig,
};
use merman_render_core::{root_svg::RootedSvg, text::TextMeasurer};
use merman_render_graph::usecase::{
    UsecaseSvgContext, render_usecase_diagram_svg_model as render_usecase,
};

#[cfg(feature = "layout-shoal")]
mod canonical;

pub(super) fn render_usecase_diagram_svg_model(
    prepared: &UsecasePreparedArtifact,
    model: &UsecaseDiagramModel,
    config: &PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let diagram_id = options.diagram_id_or("usecase");
    #[cfg(feature = "layout-shoal")]
    let paint_edges = |bounds: &mut Bounds| {
        canonical::prepare_edges(prepared, config.as_value(), bounds, options)
    };
    render_usecase(
        &prepared.prepared,
        model,
        config,
        diagram_title,
        measurer,
        &UsecaseSvgContext {
            diagram_id,
            viewport: options.root_viewport(DiagramFamilyId::USECASE, diagram_id),
            work_meter: options.work_meter(),
            operation: options.family.session().operation_context(),
            native_theme: options.native_family_theme(),
            icon_registry: options.icon_registry(),
            #[cfg(feature = "layout-shoal")]
            canonical: prepared.canonical_svg(&paint_edges),
        },
    )
}
