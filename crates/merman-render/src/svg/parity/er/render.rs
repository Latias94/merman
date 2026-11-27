//! Supplies the selected route and operation environment to the graph renderer.

use crate::{DiagramFamilyId, Result, graph_layout::RouteContract, model::ErDiagramLayout};
use merman_model::{families::er::ErDiagramModel, presentation::PresentationConfig};
use merman_render_core::{root_svg::RootedSvg, text::TextMeasurer};
use merman_render_graph::er::{self, ErEntityThemePlan, ErSvgContext};
use merman_theme_contract::v2::{Paint, Texture};

use super::super::SvgExecution;

#[cfg(feature = "layout-shoal")]
mod canonical;

#[cfg(feature = "layout-shoal")]
fn er_canonical_edge_painter<'a>(
    routes: &'a crate::graph_layout::canonical::CanonicalRoutes,
    binding: &'a er::ErCssBinding,
    work_meter: &'a crate::resources::OperationWorkMeter,
) -> impl for<'points> Fn(
    &merman_render_core::LayoutEdge,
    kurbo::Vec2,
    &roughr::core::RoughRandomness,
    &'points mut merman_render_core::route_paint::CanonicalPaintScratch,
) -> Result<er::ErCanonicalEdge<'points>>
+ 'a {
    move |edge, translation, randomness, scratch| {
        let painted = canonical::paint_edge(
            routes,
            edge,
            translation,
            binding,
            work_meter,
            randomness,
            scratch,
        )?;
        Ok(er::ErCanonicalEdge {
            d: painted.d,
            points: painted.points,
            bounds: painted.bounds,
        })
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Layout, route metadata, and the resolved terminal theme have distinct owners."
)]
pub(crate) fn render_er_diagram_svg_model(
    layout: &ErDiagramLayout,
    route_contract: &RouteContract,
    model: &ErDiagramModel,
    entity_theme: &ErEntityThemePlan,
    effective_config: &PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    let binding = entity_theme.css_binding();
    let work_meter = options.work_meter();
    #[cfg(feature = "layout-shoal")]
    let canonical_routes = route_contract
        .canonical_routes()
        .map(|routes| er_canonical_edge_painter(routes, binding, work_meter));
    let rough_randomness =
        options.rough_randomness(binding.render_settings.hand_drawn_seed, "render.er.roughjs");
    let session = options.family.session();
    let canvas_background = session.canvas_background().or_else(|| {
        options
            .native_family_theme()
            .and_then(|theme| match theme.roles.canvas {
                Paint::Solid(color) => Some(color),
                Paint::LinearGradient(_) => None,
            })
    });
    let canvas_has_texture = session.native_theme().is_some_and(|theme| {
        theme
            .resolved()
            .canvas()
            .textures
            .iter()
            .any(|texture| !matches!(texture, Texture::None))
    });
    let context = ErSvgContext {
        diagram_id,
        viewport: options.root_viewport(DiagramFamilyId::ER, diagram_id),
        canvas_background,
        canvas_has_texture,
        elk_parity: route_contract.is_elk_parity(),
        swimlane: matches!(route_contract, RouteContract::Swimlane),
        #[cfg(feature = "layout-grid")]
        grid: match route_contract {
            RouteContract::Grid(paint) => Some(*paint),
            _ => None,
        },
        #[cfg(feature = "layout-shoal")]
        canonical_routes: canonical_routes
            .as_ref()
            .map(|paint| paint as &er::ErCanonicalEdgePainter<'_>),
        work_meter,
        rough_randomness: &rough_randomness,
        include_edges: options.debug.include_edges,
        viewbox_padding: options.viewbox_padding,
    };
    er::render_er_diagram_svg_model(
        layout.svg_layout(),
        model,
        entity_theme,
        effective_config,
        diagram_title,
        measurer,
        &context,
    )
}
