//! Supplies Class layout routes and operation context to the graph SVG writer.

use super::SvgExecution;
use crate::{DiagramFamilyId, Result, graph_layout::RouteContract, model::ClassDiagramLayout};
use merman_model::{families::class::ClassDiagram, presentation::PresentationConfig};
use merman_render_core::{root_svg::RootedSvg, text::TextMeasurer};
use merman_render_graph::class::{
    self, ClassNodeVisualPlan, ClassPreparedLabels, ClassRelationThemePlan, ClassRenderConfig,
    ClassSvgContext, ClassTextThemePlan,
};

#[cfg(feature = "layout-shoal")]
pub(crate) use class::class_canonical_marker;

#[allow(
    clippy::too_many_arguments,
    reason = "The Class writer receives prepared geometry, styles, and operation inputs."
)]
pub(super) fn render_class_diagram_svg_model_with_config(
    layout: &ClassDiagramLayout,
    prepared_labels: &ClassPreparedLabels,
    route_contract: &RouteContract,
    model: &ClassDiagram,
    relation_theme: &ClassRelationThemePlan,
    typography_theme: &ClassTextThemePlan,
    node_visual_plan: &ClassNodeVisualPlan,
    render_config: &ClassRenderConfig,
    effective_config: &PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    let work_meter = options.work_meter();
    #[cfg(feature = "layout-shoal")]
    let paint = route_contract.canonical_routes().map(|routes| {
        move |edge: &merman_render_core::LayoutEdge,
              translation,
              stroke_outset,
              style,
              scratch: &mut merman_render_core::route_paint::CanonicalPaintScratch| {
            let painted = routes.paint_with_scratch(
                edge,
                translation,
                stroke_outset,
                style,
                work_meter,
                scratch,
            )?;
            Ok(class::ClassCanonicalEdge {
                path: painted.path,
                marker_points: painted.marker_points,
                data_points: painted.data_points,
                markers: painted.markers,
            })
        }
    });
    #[cfg(feature = "layout-shoal")]
    let canonical_routes = paint.as_ref().map(|paint| class::ClassCanonicalRoutes {
        paint,
        rough_stroke: merman_render_core::rough_svg::constrained_rough_edge_path,
    });
    let context = ClassSvgContext {
        diagram_id,
        viewport: options.root_viewport(DiagramFamilyId::CLASS, diagram_id),
        work_meter,
        math_renderer: options.math_renderer(),
        timing: options.timing(),
        hand_drawn_seed: options.rough_randomness(
            render_config
                .hand_drawn_seed()
                .unwrap_or(options.seed() as f64),
            "render.class.roughjs",
        ),
        elk_parity: route_contract.is_elk_parity(),
        #[cfg(feature = "layout-grid")]
        grid: match route_contract {
            RouteContract::Grid(paint) => Some(*paint),
            _ => None,
        },
        #[cfg(feature = "layout-shoal")]
        canonical_routes: canonical_routes.as_ref(),
    };
    class::render_class_diagram_svg_model_with_config(
        layout,
        prepared_labels,
        model,
        relation_theme,
        typography_theme,
        node_visual_plan,
        render_config,
        effective_config,
        diagram_title,
        measurer,
        context,
    )
}
