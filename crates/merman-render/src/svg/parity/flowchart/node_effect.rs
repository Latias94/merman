//! Node-owned shadow geometry shared by viewport preparation and terminal emission.

use rustc_hash::FxHashMap;

use super::render::node::helpers::resolve_node_render_info;
use super::*;
use crate::diagram_theme::{MaterializedShadowEffect, ThemeResourcePolicy};
use crate::flowchart::{
    FlowchartFacetPrecedence, FlowchartNodeThemeStyle, FlowchartShape, FlowchartSourceFacetStatus,
};

#[derive(Debug)]
pub(super) struct PreparedNodeEffect {
    pub(super) style: FlowchartNodeThemeStyle,
    pub(super) source: FlowchartCompiledStyles,
    pub(super) shadow: Option<MaterializedShadowEffect>,
    bounds: Option<(f64, f64, f64, f64)>,
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
        for id in hierarchy.rendered_node_ids() {
            ctx.emit.checkpoint()?;
            ctx.work_meter.charge(1)?;
            let ordinal = ctx.node_theme_ordinals.get(id).copied();
            let Some(node) = ctx.layout_nodes_by_id.get(id) else {
                continue;
            };
            let Some(info) = resolve_node_render_info(ctx, id) else {
                continue;
            };
            let style =
                FlowchartNodeThemeStyle::resolve(ctx.resolved_theme, ordinal, ctx.work_meter)?;
            let source = flowchart_compile_node_styles(
                ctx.class_defs,
                info.node_classes,
                info.node_styles,
                &[],
            );
            let source_filter = source.source_filter_status();
            let source_stroke_width = source.source_stroke_width_status();
            let stroke_precedence = FlowchartFacetPrecedence::new(
                source_stroke_width,
                ctx.node_stroke_width_config_override,
            );
            let stroke = source
                .admitted_stroke_width_value()
                .or_else(|| {
                    ctx.node_stroke_width_config_override
                        .then_some(ctx.node_stroke_width)
                })
                .or_else(|| style.stroke_width_value(stroke_precedence, true))
                .unwrap_or(ctx.node_stroke_width);
            let geometry = if style.effect().is_some() {
                node_shadow_geometry(info.shape, node.width, node.height)?
            } else {
                None
            };
            // Relative CSS widths require host geometry. Do not attach a filter whose region
            // would clip the source by substituting the renderer default stroke width.
            let shadow = if source_filter.is_absent()
                && source_stroke_width != FlowchartSourceFacetStatus::Unverified
                && flowchart_config_look(ctx.config) == "classic"
            {
                if let (Some(effect), Some((_, _, width, height))) = (style.effect(), geometry) {
                    let half_stroke = f64::from(stroke) / 2.0;
                    let (horizontal, vertical) =
                        if FlowchartShape::resolve(info.shape)? == FlowchartShape::Diamond {
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
                .insert(
                    id.to_owned(),
                    PreparedNodeEffect {
                        style,
                        source,
                        shadow,
                        bounds,
                    },
                )
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
            .filter(|node| node.shadow.is_some())
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

// These classic emitters paint a complete shape before the separate label writer runs. Other
// shapes retain an explicit residual until their actual paint geometry has a terminal adapter.
fn node_shadow_geometry(
    shape: &str,
    width: f64,
    height: f64,
) -> crate::Result<Option<(f64, f64, f64, f64)>> {
    let width = width.max(1.0);
    let height = height.max(1.0);
    Ok(match FlowchartShape::resolve(shape)? {
        FlowchartShape::Process | FlowchartShape::RoundedRectangle => {
            Some((-width / 2.0, -height / 2.0, width, height))
        }
        FlowchartShape::Diamond => Some((-width / 2.0 + 0.5, -height / 2.0, width, height)),
        FlowchartShape::Circle | FlowchartShape::DoubleCircle => {
            let diameter = width.min(height).max(1.0);
            Some((-diameter / 2.0, -diameter / 2.0, diameter, diameter))
        }
        _ => None,
    })
}
