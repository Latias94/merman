//! Bridges the operation environment and canonical routes to the Architecture writer.

use crate::{
    DiagramFamilyId, Result, architecture::ArchitectureGroupThemePlan,
    model::ArchitectureDiagramLayout,
};
use merman_model::{
    families::architecture::ArchitectureDiagramModel, presentation::PresentationConfig,
};
use merman_render_core::root_svg::RootedSvg;
use merman_render_graph::architecture::{self, ArchitectureSvgContext};

use super::SvgExecution;

#[cfg(all(test, feature = "layout-shoal"))]
mod shoal_tests;

pub(super) fn render_architecture_diagram_svg_typed_with_config(
    layout: &ArchitectureDiagramLayout,
    model: &ArchitectureDiagramModel,
    effective_config: &PresentationConfig,
    group_theme: &ArchitectureGroupThemePlan,
    #[cfg(feature = "layout-shoal")] shoal_geometry: Option<
        &crate::architecture::ArchitectureShoalGeometry,
    >,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let diagram_id = options.diagram_id_or("architecture");
    let context = ArchitectureSvgContext {
        diagram_id,
        viewport: options.root_viewport(DiagramFamilyId::ARCHITECTURE, diagram_id),
        work_meter: options.work_meter(),
        text_measurer: options.text_measurer(),
        icon_registry: options.icon_registry(),
        timing: options.timing(),
        #[cfg(feature = "layout-shoal")]
        canonical: shoal_geometry
            .map(|geometry| geometry as &dyn architecture::ArchitectureCanonicalSvg),
    };
    architecture::render_architecture_diagram_svg_typed_with_config(
        layout,
        model,
        effective_config,
        group_theme,
        &context,
    )
}

#[cfg(feature = "layout-shoal")]
mod canonical {
    use crate::{
        Result,
        architecture::{ArchitectureShoalGeometry, labels::SvgLine},
        graph_layout::route_paint::RouteStyle,
    };
    use merman_render_core::{
        LayoutEdge, LayoutLabel, LayoutPoint, resources::OperationWorkMeter,
        route_paint::CanonicalPaintScratch,
    };
    use merman_render_graph::architecture::{
        ArchitectureCanonicalEdge, ArchitectureCanonicalGroup, ArchitectureCanonicalSvg,
        ArchitecturePlacedText,
    };

    impl ArchitectureCanonicalSvg for ArchitectureShoalGeometry {
        fn group_count(&self) -> usize {
            self.groups.len()
        }

        fn group(&self, ordinal: usize) -> ArchitectureCanonicalGroup {
            let group = &self.groups[ordinal];
            ArchitectureCanonicalGroup {
                parent: group.parent,
                x: group.rect.x,
                y: group.rect.y,
                width: group.rect.width,
                height: group.rect.height,
                icon_origin: group.icon_origin.map(|p| LayoutPoint { x: p.x, y: p.y }),
            }
        }

        fn service_title(&self, ordinal: usize) -> Option<(&[SvgLine], LayoutPoint)> {
            let title = self.service_titles[ordinal].as_ref()?;
            let anchor = LayoutPoint {
                x: title.anchor.x,
                y: title.anchor.y,
            };
            Some((&title.text.lines, anchor))
        }

        fn group_title(&self, ordinal: usize) -> Option<ArchitecturePlacedText<'_>> {
            let title = self.groups[ordinal].title.as_ref()?;
            Some(ArchitecturePlacedText {
                lines: &title.text.lines,
                anchor: LayoutPoint {
                    x: title.anchor.x,
                    y: title.anchor.y,
                },
                bounds: title.text.bounds_at(title.anchor),
            })
        }

        fn edge_label(
            &self,
            ordinal: usize,
            label: Option<&LayoutLabel>,
        ) -> Option<ArchitecturePlacedText<'_>> {
            let (label, text) = label.zip(self.edge_labels[ordinal].as_ref())?;
            let anchor = text.centered_at(label);
            Some(ArchitecturePlacedText {
                lines: &text.lines,
                anchor: LayoutPoint {
                    x: anchor.x,
                    y: anchor.y,
                },
                bounds: text.bounds_at(anchor),
            })
        }

        fn paint_edge<'points>(
            &self,
            edge: &LayoutEdge,
            work_meter: &OperationWorkMeter,
            scratch: &'points mut CanonicalPaintScratch,
        ) -> Result<ArchitectureCanonicalEdge<'points>> {
            let route_style = match self.route_style {
                // Bound the bend within the grid's right-angle channels.
                RouteStyle::Curved => RouteStyle::Rounded(10.0),
                style => style,
            };
            let painted = self.routes.paint_with_borrowed_points(
                edge,
                kurbo::Vec2::ZERO,
                self.edge_stroke_outset,
                route_style,
                work_meter,
                scratch,
            )?;
            Ok(ArchitectureCanonicalEdge {
                path: painted.path,
                data_points: painted.data_points,
                marker_points: painted.marker_points,
                markers: painted.markers,
            })
        }
    }
}
