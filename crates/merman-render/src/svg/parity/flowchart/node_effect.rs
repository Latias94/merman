//! Winning node effects and paint geometry shared by bounds and terminal emission.

use rustc_hash::FxHashMap;

use super::render::node::helpers::resolve_node_render_info;
use super::*;
use crate::diagram_theme::{MaterializedShadowEffect, ThemeResourcePolicy};
use crate::flowchart::{FlowchartShape, FlowchartSourceFacetStatus};

#[derive(Debug, Clone, Copy)]
pub(super) enum PreparedNodeFilter {
    Retain,
    Clear,
    Shadow(MaterializedShadowEffect),
}

#[derive(Debug)]
pub(super) struct PreparedNodeEffect {
    pub(super) filter: PreparedNodeFilter,
    bounds: Option<(f64, f64, f64, f64)>,
}

impl PreparedNodeEffect {
    pub(super) fn shadow(&self) -> Option<MaterializedShadowEffect> {
        match self.filter {
            PreparedNodeFilter::Shadow(shadow) => Some(shadow),
            _ => None,
        }
    }

    pub(super) fn replaces_look_filter(&self) -> bool {
        !matches!(self.filter, PreparedNodeFilter::Retain)
    }
}

#[derive(Debug, Default)]
pub(super) struct FlowchartNodeEffects {
    nodes: FxHashMap<String, PreparedNodeEffect>,
}

impl FlowchartNodeEffects {
    pub(super) fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy: &FlowchartHierarchyPlan<'_>,
        resources: &ThemeResourcePolicy,
    ) -> crate::Result<Self> {
        let mut plan = Self::default();
        let look = ctx.compatibility.look.as_str();
        let supported_look = look == "classic" || super::supports_neo_effects(ctx);
        let structural_source_bounded = super::style::node_shadow_structural_styles_are_bounded(
            ctx.class_defs,
            &["root", "nodes", "node"],
        );
        let double_circle_source_bounded = structural_source_bounded
            && super::style::node_shadow_structural_styles_are_bounded(
                ctx.class_defs,
                &["basic", "label-container"],
            );
        for id in hierarchy.rendered_node_ids() {
            ctx.emit.checkpoint()?;
            if ctx.document_ids().synthetic_label(id).is_some() {
                continue;
            }
            let Some(node) = ctx.layout_nodes_by_id.get(id) else {
                continue;
            };
            let Some(info) = resolve_node_render_info(ctx, id) else {
                continue;
            };
            let Some(prepared) = ctx.prepared_nodes.node(id) else {
                continue;
            };
            let style = &prepared.style;
            let source = &prepared.source;
            let selection = &prepared.selection;
            let source_filter = source.source_filter_status();
            let source_stroke_width = source.source_stroke_width_status();
            let shape = FlowchartShape::resolve(info.shape)?;
            let source_geometry_bounded = source.node_shadow_source_is_bounded()
                && if shape == FlowchartShape::DoubleCircle {
                    double_circle_source_bounded
                } else {
                    structural_source_bounded
                };
            let geometry = if style.effect().is_some() || style.effect_is_cleared() {
                node_shadow_geometry(shape, node.width, node.height, look == "neo")
            } else {
                None
            };
            // Relative CSS widths require host geometry. Do not attach a filter whose region
            // would clip the source by substituting the renderer default stroke width.
            let shadow = if source_filter.is_absent()
                && source_geometry_bounded
                && source_stroke_width != FlowchartSourceFacetStatus::Unverified
                && supported_look
            {
                if let (Some(effect), Some((_, _, width, height))) = (style.effect(), geometry) {
                    let half_stroke = f64::from(selection.effect_stroke_width) / 2.0;
                    let (horizontal, vertical) = if shape == FlowchartShape::Diamond {
                        // Use the full miter envelope. A smaller source miter limit or a round/
                        // bevel join can only reduce it, never clip the actual source stroke.
                        let diagonal = width.hypot(height);
                        (
                            half_stroke * diagonal / height,
                            half_stroke * diagonal / width,
                        )
                    } else {
                        (half_stroke, half_stroke)
                    };
                    effect.materialize_with_source_outsets(
                        resources,
                        width,
                        height,
                        crate::diagram_theme::EffectOutsets {
                            top: vertical,
                            right: horizontal,
                            bottom: vertical,
                            left: horizontal,
                        },
                    )?
                } else {
                    None
                }
            } else {
                None
            };
            let filter = if let Some(shadow) = shadow {
                PreparedNodeFilter::Shadow(shadow)
            } else if style.effect_is_cleared()
                && source_filter.is_absent()
                && source_geometry_bounded
                && (geometry.is_some() && supported_look
                    || look == "classic"
                        && !matches!(
                            shape,
                            FlowchartShape::SmallCircle | FlowchartShape::FramedCircle
                        ))
            {
                // Classic shapes without a body filter need no physical suppression. Neo
                // needs a terminal that can replace all default filters for that silhouette.
                PreparedNodeFilter::Clear
            } else {
                PreparedNodeFilter::Retain
            };
            let bounds = shadow.zip(geometry).map(|(shadow, (x, y, width, height))| {
                let outsets = shadow.outsets();
                (
                    x - outsets.left,
                    y - outsets.top,
                    x + width + outsets.right,
                    y + height + outsets.bottom,
                )
            });
            if plan
                .nodes
                .insert(id.to_owned(), PreparedNodeEffect { filter, bounds })
                .is_some()
            {
                return Err(crate::Error::InvalidModel {
                    message: format!("Flowchart emits themed node `{id}` more than once"),
                });
            }
        }
        Ok(plan)
    }

    pub(super) fn node(&self, id: &str) -> Option<&PreparedNodeEffect> {
        self.nodes.get(id)
    }

    pub(super) fn expected_applications(&self) -> usize {
        self.nodes
            .values()
            .filter(|node| node.shadow().is_some())
            .count()
    }

    pub(super) fn has_paint_bounds(&self) -> bool {
        self.nodes.values().any(|node| node.bounds.is_some())
    }

    pub(super) fn include_bounds(
        &self,
        id: &str,
        x: f64,
        y: f64,
        include: &mut impl FnMut(f64, f64, f64, f64),
    ) {
        if let Some((min_x, min_y, max_x, max_y)) = self.nodes.get(id).and_then(|node| node.bounds)
        {
            include(x + min_x, y + min_y, x + max_x, y + max_y);
        }
    }
}

// These terminals paint a complete shape before the separate label writer runs. Other
// shapes retain an explicit residual until their actual paint geometry has a terminal adapter.
fn node_shadow_geometry(
    shape: FlowchartShape,
    width: f64,
    height: f64,
    neo: bool,
) -> Option<(f64, f64, f64, f64)> {
    let width = width.max(1.0);
    let height = height.max(1.0);
    match shape {
        FlowchartShape::Process | FlowchartShape::RoundedRectangle => {
            Some((-width / 2.0, -height / 2.0, width, height))
        }
        FlowchartShape::Diamond => Some((-width / 2.0 + 0.5, -height / 2.0, width, height)),
        FlowchartShape::Circle => {
            let diameter = width.min(height).max(1.0);
            Some((-diameter / 2.0, -diameter / 2.0, diameter, diameter))
        }
        FlowchartShape::DoubleCircle => {
            let geometry =
                crate::flowchart::DoubleCircleGeometry::from_outer_diameter(width.min(height), neo);
            let radius = geometry.outer_radius;
            Some((-radius, -radius, radius * 2.0, radius * 2.0))
        }
        _ => None,
    }
}
