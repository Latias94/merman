//! Text-only filter plans share measured paint bounds with the viewport and label writers.

use rustc_hash::FxHashMap;

use super::*;
use crate::diagram_theme::{
    EffectOutsets, MaterializedShadowEffect, SvgShadowEffect, ThemeResourcePolicy, ThemeTarget,
};
use crate::flowchart::{FlowchartNodeThemeStyle, FlowchartShape, FlowchartSvgLabelOwner};

#[derive(Debug)]
pub(super) struct PreparedLabelEffect {
    effect: SvgShadowEffect,
    shadow: MaterializedShadowEffect,
    // Both preparation and emission use the existing label translation, not its ink center.
    translation: (f64, f64),
    absolute_translation: (f64, f64),
}

impl PreparedLabelEffect {
    pub(super) fn open(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        id: &str,
        translation: (f64, f64),
    ) -> bool {
        if self.translation != translation {
            return false;
        }
        let reference = crate::svg::parity::shadow::write_theme_shadow_application(
            out,
            id,
            &self.effect,
            self.shadow.region(),
        );
        let _ = write!(out, r#"<g filter="{}">"#, escape_attr(&reference));
        true
    }

    pub(super) fn close(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        ctx: &FlowchartRenderCtx<'_>,
        id: &str,
    ) {
        out.push_str("</g>");
        ctx.effect_evidence
            .record_application(&self.effect, id, self.shadow.region());
    }
}

#[derive(Debug, Default)]
pub(super) struct FlowchartLabelEffects {
    nodes: FxHashMap<String, PreparedLabelEffect>,
    edges: FxHashMap<crate::flowchart::FlowchartEdgeKey, PreparedLabelEffect>,
}

impl FlowchartLabelEffects {
    pub(super) fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy: &FlowchartHierarchyPlan<'_>,
        render_edges: &[super::render_input::FlowchartRenderEdge<'_>],
        edge_cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
        resources: &ThemeResourcePolicy,
    ) -> crate::Result<Self> {
        let mut plan = Self::default();
        let (Some(theme), Some(sidecar)) = (ctx.resolved_theme, ctx.svg_label_sidecar) else {
            return Ok(plan);
        };
        if ctx.swimlane_direction.is_some() || flowchart_config_look(ctx.config) != "classic" {
            return Ok(plan);
        }
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                crate::diagram_theme::FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel,
                    ..
                } | crate::diagram_theme::FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel,
                    facet: crate::diagram_theme::FamilyThemeRuleFacet::Effect,
                    ..
                }
            )
        }) {
            return Ok(plan);
        }
        let weights = sidecar.label_weights();
        if !ctx.node_html_labels
            && super::style::label_shadow_structural_styles_are_bounded(
                ctx.class_defs,
                &[
                    "root",
                    "nodes",
                    "node",
                    "label",
                    "nodeLabel",
                    "text-outer-tspan",
                    "row",
                ],
            )
        {
            for id in hierarchy.rendered_node_ids() {
                let Some(info) = super::render::node::helpers::resolve_node_render_info(ctx, id)
                else {
                    continue;
                };
                // These writers leave the ordinary label translation unchanged. Special shape
                // label placement remains residual until its own preparation shares that position.
                if info.label_type == "markdown"
                    || !matches!(
                        FlowchartShape::resolve(info.shape)?,
                        FlowchartShape::Process
                            | FlowchartShape::RoundedRectangle
                            | FlowchartShape::Diamond
                            | FlowchartShape::Circle
                            | FlowchartShape::DoubleCircle
                    )
                {
                    continue;
                }
                let Some(node) = ctx.layout_nodes_by_id.get(id) else {
                    continue;
                };
                let Some(height) = node.label_height else {
                    continue;
                };
                let style = FlowchartNodeThemeStyle::resolve(
                    Some(theme),
                    ctx.node_theme_ordinals.get(id).copied(),
                    ctx.work_meter,
                )?;
                let Some(effect) = style.label_effect() else {
                    continue;
                };
                let source = flowchart_compile_node_styles(
                    ctx.class_defs,
                    info.node_classes,
                    info.node_styles,
                    &[],
                );
                if !source.label_shadow_source_is_bounded() {
                    continue;
                }
                let base_style = if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
                    &ctx.html_label_text_style
                } else {
                    &ctx.text_style
                };
                let text_style = weights.apply_cow(
                    ThemeTarget::NodeLabel,
                    crate::flowchart::flowchart_effective_text_style_for_node_classes(
                        base_style,
                        ctx.class_defs,
                        info.node_classes,
                        info.node_styles,
                    ),
                );
                let Some(owner) = sidecar.node_owner(id, false) else {
                    continue;
                };
                let raw = if info.label_text_is_node_id {
                    id
                } else {
                    info.label_text
                };
                let Some(bounds) = sidecar.centered_shadow_bounds(owner, raw, text_style.as_ref())
                else {
                    continue;
                };
                let translation = (0.0, -height / 2.0);
                let root_offset = hierarchy
                    .effective_parent(id)
                    .and_then(|root| hierarchy.root_offsets(root));
                let y_shift =
                    root_offset.map_or(0.0, |offset| offset.abs_top_transform - offset.origin_y);
                if let Some(prepared) = prepare_effect(
                    effect,
                    resources,
                    bounds,
                    translation,
                    (
                        node.x + ctx.tx + translation.0,
                        node.y + ctx.ty + y_shift + translation.1,
                    ),
                )? {
                    ctx.work_meter.charge(1)?;
                    plan.nodes.insert(id.to_owned(), prepared);
                }
            }
        }
        if !ctx.edge_html_labels
            && super::style::label_shadow_structural_styles_are_bounded(
                ctx.class_defs,
                &[
                    "root",
                    "edgeLabels",
                    "edgeLabel",
                    "label",
                    "text-outer-tspan",
                    "row",
                ],
            )
            && let Some(effect) = ctx.edge_theme.label_effect()
        {
            for render_edge in render_edges {
                let edge = render_edge.as_ref();
                if edge.edge.label_type.as_deref() == Some("markdown") {
                    continue;
                }
                let Some(layout_edge) = ctx.layout_edges_by_key.get(&edge.key) else {
                    continue;
                };
                let Some(label) = layout_edge.label.as_ref() else {
                    continue;
                };
                let source = ctx.edge_style_plan.edge_for(edge.key)?;
                if !source.label_shadow_source_is_bounded() {
                    continue;
                }
                let raw = ctx
                    .model
                    .edge_label_for_render(edge.key.semantic_index(), edge.edge)
                    .unwrap_or_default();
                let text_style = weights.apply_cow(
                    ThemeTarget::EdgeLabel,
                    source.effective_edge_label_text_style(&ctx.text_style),
                );
                let Some(bounds) = sidecar.centered_shadow_bounds(
                    FlowchartSvgLabelOwner::Edge(edge.key.semantic_index()),
                    raw,
                    text_style.as_ref(),
                ) else {
                    continue;
                };
                let root = hierarchy.edge_root(edge.key)?.unwrap_or("");
                let offsets = hierarchy
                    .root_offsets(root)
                    .unwrap_or(FlowchartRootOffsets {
                        origin_x: 0.0,
                        origin_y: 0.0,
                        abs_top_transform: 0.0,
                    });
                let position = super::render::edge_label::resolve_flowchart_edge_label_position(
                    ctx,
                    edge.key,
                    layout_edge,
                    label,
                    offsets.origin_x,
                    offsets.origin_y,
                    edge_cache,
                    false,
                );
                let plain = flowchart_label_plain_text(
                    raw,
                    edge.edge.label_type.as_deref().unwrap_or("text"),
                    false,
                );
                let label_box = super::render::edge_label::flowchart_svg_edge_label_box(
                    label.width,
                    label.height,
                    ctx.measurer
                        .measure_svg_create_text_bbox_y_offset_px(&plain, text_style.as_ref()),
                    ctx.edge_label_padding,
                );
                let translation = (label_box.translate_x, label_box.translate_y);
                if let Some(prepared) = prepare_effect(
                    effect,
                    resources,
                    bounds,
                    translation,
                    (
                        position.x + offsets.origin_x + translation.0,
                        position.y + offsets.abs_top_transform + translation.1,
                    ),
                )? {
                    ctx.work_meter.charge(1)?;
                    plan.edges.insert(edge.key, prepared);
                }
            }
        }
        Ok(plan)
    }

    pub(super) fn node(&self, id: &str) -> Option<&PreparedLabelEffect> {
        self.nodes.get(id)
    }
    pub(super) fn edge(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> Option<&PreparedLabelEffect> {
        self.edges.get(&key)
    }
    pub(super) fn expected_applications(&self) -> usize {
        self.nodes.len() + self.edges.len()
    }
    pub(super) fn include_bounds(&self, include: &mut impl FnMut(f64, f64, f64, f64)) {
        for prepared in self.nodes.values().chain(self.edges.values()) {
            let [x, y, width, height] = prepared.shadow.region().as_array().map(f64::from);
            let (tx, ty) = prepared.absolute_translation;
            include(x + tx, y + ty, x + width + tx, y + height + ty);
        }
    }
}

fn prepare_effect(
    effect: &SvgShadowEffect,
    resources: &ThemeResourcePolicy,
    bounds: [f64; 4],
    translation: (f64, f64),
    absolute_translation: (f64, f64),
) -> crate::Result<Option<PreparedLabelEffect>> {
    let [min_x, min_y, max_x, max_y] = bounds;
    Ok(effect
        .materialize_user_space(
            resources,
            min_x,
            min_y,
            max_x,
            max_y,
            EffectOutsets::default(),
        )?
        .map(|shadow| PreparedLabelEffect {
            effect: effect.clone(),
            shadow,
            translation,
            absolute_translation,
        }))
}
