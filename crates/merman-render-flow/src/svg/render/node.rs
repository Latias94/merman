//! Flowchart node renderer.

use super::super::*;
use merman_render_core::svg_output::timing::RenderTiming;

mod depth;
pub(in crate::svg) mod geom;
pub(in crate::svg) mod helpers;
mod label;
pub(in crate::svg) use depth::{native_silhouette, render_flowchart_node_depth};
pub(in crate::svg) use merman_render_core::rough_svg::paths as roughjs;
pub(in crate::svg) mod shapes;

pub(in crate::svg::render) struct FlowchartNodeRenderCommon<'a> {
    geometry: FlowchartNodeGeometry<'a>,
    label_emission: &'a label::FlowchartNodeLabelEmissionPlan<'a>,
    pub style: &'a str,
    pub junction_style: &'a str,
    pub effect_filter_style: &'a str,
    /// Theme-only declarations for no-label surfaces that do not consume the complete source
    /// style string (notably flowchart-v2 start nodes).
    pub theme_style: &'a str,
    pub rough_group_style: &'a str,
    pub solid_rough_fill: bool,
    pub fill_color: &'a str,
    pub fill_opacity: f64,
    pub stroke_color: &'a str,
    pub wrapped_in_a: bool,
    pub timing: RenderTiming,
}
pub(in crate::svg::render) struct FlowchartNodeGeometry<'a> {
    pub node_id: &'a str,
    pub shape: &'a str,
    pub look: &'a str,
    pub layout_node: &'a merman_render_core::LayoutNode,
    pub canonical_axis: bool,
    pub node_classes: &'a [String],
    pub node_styles: &'a [String],
    pub node_icon: Option<&'a str>,
    pub node_img: Option<&'a str>,
    pub node_pos: Option<&'a str>,
    pub node_constraint: Option<&'a str>,
    pub node_asset_width: Option<f64>,
    pub node_asset_height: Option<f64>,
    pub stroke_width: f32,
    pub stroke_dasharray: &'a str,
    pub native_corner_radius: Option<f64>,
    pub source_corner_radii: [Option<f64>; 2],
    pub configured_corner_radius: Option<f64>,
    pub hand_drawn_seed: &'a roughr::core::RoughRandomness,
    pub work_meter: &'a merman_render_core::resources::OperationWorkMeter,
    pub source_text_style: &'a merman_render_core::text::TextStyle,
}

impl FlowchartNodeGeometry<'_> {
    pub(super) fn look_is_neo(&self) -> bool {
        self.look == "neo"
    }
    pub(super) fn look_is_hand_drawn(&self) -> bool {
        self.look == "handDrawn"
    }
    pub(super) fn label_metrics(
        &self,
        ctx: &FlowchartRenderCtx<'_>,
        label: &FlowchartNodeLabelState<'_>,
    ) -> merman_render_core::text::TextMetrics {
        helpers::compute_node_label_metrics_with_style(
            ctx,
            Some(self.layout_node),
            label.text,
            label.label_type,
            self.source_text_style,
        )
    }
}

impl<'a> std::ops::Deref for FlowchartNodeRenderCommon<'a> {
    type Target = FlowchartNodeGeometry<'a>;
    fn deref(&self) -> &Self::Target {
        &self.geometry
    }
}

pub(in crate::svg::render) struct FlowchartNodeLabelState<'a> {
    pub text: &'a str,
    pub label_type: &'a str,
    pub dx: f64,
    pub dy: f64,
}

#[allow(
    clippy::too_many_arguments,
    reason = "The node renderer borrows the document's label-style buffer separately from timing details."
)]
pub fn render_flowchart_node(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
    timing: RenderTiming,
    details: &mut FlowchartRenderDetails,
    div_style_scratch: &mut String,
) -> merman_render_core::Result<()> {
    let Some(layout_node) = ctx.layout_nodes_by_id.get(node_id) else {
        return Ok(());
    };

    let x = layout_node.x + ctx.tx - origin_x;
    let y = layout_node.y + ctx.ty - origin_y;

    if let Some(dom_id) = ctx.document_ids().synthetic_label(node_id)
        && helpers::try_render_self_loop_label_placeholder(
            out,
            node_id,
            dom_id,
            x,
            y,
            ctx.node_html_labels,
        )
    {
        return Ok(());
    }

    let Some(resolved) = helpers::resolve_node_render_info(ctx, node_id) else {
        return Ok(());
    };

    let tooltip = ctx.tooltips.get(node_id).map(|s| s.as_str()).unwrap_or("");
    let tooltip_enabled = !tooltip.trim().is_empty();

    let look = ctx.compatibility.look.as_str();
    let node_dom_id = ctx.document_ids().node(node_id).ok_or_else(|| {
        merman_render_core::Error::InvalidModel {
            message: format!("missing prepared Flowchart DOM id for node `{node_id}`"),
        }
    })?;
    let class_attr_base = if look == "handDrawn" {
        match resolved.class_attr_base {
            "node default" => "rough-node default",
            "node" => "rough-node",
            other => other,
        }
    } else {
        resolved.class_attr_base
    };
    let wrapped_in_a = resolved.wrapped_in_a;
    let href = resolved.href.as_ref();
    let target = resolved.target;
    let shape: &str = resolved.shape;
    let node_classes = resolved.node_classes;
    let bpmn_classes = crate::bpmn::BpmnShape::resolve(shape).map(|shape| shape.classes());

    let empty_classes: &[String] = &[];
    let node_classes_for_wrapper = match shape {
        // Mermaid flowchart-v2 start/stop nodes do not carry classDef classes on the wrapper.
        // Styling is applied via inline styles on the shape paths (stop) or ignored (start).
        "sm-circ" | "small-circle" | "start" | "fr-circ" | "framed-circle" | "stop" => {
            empty_classes
        }
        _ => node_classes,
    };
    let wrapper_classes =
        helpers::NodeWrapperClasses::new(class_attr_base, node_classes_for_wrapper)
            .with_extra(bpmn_classes);
    helpers::open_node_wrapper(
        out,
        helpers::NodeWrapperAttrs {
            dom_id: node_dom_id,
            data_id: node_id,
            classes: wrapper_classes,
            wrapped_in_a,
            href,
            target,
            x,
            y,
            tooltip_enabled,
            tooltip,
            look,
            color_slot: super::super::agentflow::container_color_slot(ctx, node_id),
        },
    );
    ctx.checkpoint_emit()?;

    let style_start = timing.start();
    let prepared = ctx.prepared_nodes.node(node_id).ok_or_else(|| {
        merman_render_core::Error::InvalidModel {
            message: format!("Flowchart node `{node_id}` has no prepared terminal style"),
        }
    })?;
    let compiled_styles = &prepared.source;
    let selection = &prepared.selection;
    let style = selection
        .inline_style
        .as_deref()
        .unwrap_or(&prepared.source.node_style);
    if let Some(s) = style_start {
        details.node_style_compile += s.elapsed();
    }
    let native_label_color = selection.native_label_color.as_deref();
    let swimlane_label_color =
        if ctx.swimlane_direction.is_some() && !prepared.has_structural_label_foreground {
            super::cluster::swimlane_label_ink(
                ctx,
                node_id,
                compiled_styles,
                None,
                native_label_color.unwrap_or(&ctx.compatibility.node_text_color),
                prepared.source_text_style.as_ref(),
            )?
        } else {
            None
        };
    let authored_svg_color =
        if !ctx.node_html_labels && ctx.layout.route_contract.is_canonical_axis() {
            compiled_styles.source_label_color()
        } else {
            None
        };
    let fill_color = prepared.fill_color(ctx.compatibility);
    let stroke_color = prepared.stroke_color(ctx.compatibility);
    let label_emission = label::FlowchartNodeLabelEmissionPlan::new(
        prepared.source_text_style.as_ref(),
        compiled_styles,
        authored_svg_color
            .or(swimlane_label_color.as_deref())
            .or(native_label_color),
    );

    let suppress_look_filter =
        ctx.native.is_some() && !compiled_styles.has_source_filter() && look == "neo";
    let effect_filter_style = if suppress_look_filter {
        ";filter:none !important"
    } else {
        ""
    };
    let common = FlowchartNodeRenderCommon {
        geometry: prepare_node_geometry(ctx, node_id, &resolved)?,

        label_emission: &label_emission,
        style,
        junction_style: selection.junction_style.as_deref().unwrap_or(style),
        effect_filter_style,
        theme_style: &selection.theme_style,
        rough_group_style: &prepared.rough_group_style,
        solid_rough_fill: ctx.layout.route_contract.is_canonical_axis(),
        fill_color,
        fill_opacity: compiled_styles.fill_opacity,
        stroke_color,
        wrapped_in_a,
        timing,
    };
    let mut label = FlowchartNodeLabelState {
        text: if resolved.label_text_is_node_id {
            node_id
        } else {
            resolved.label_text
        },
        label_type: resolved.label_type,
        dx: 0.0,
        dy: 0.0,
    };

    if shape == "collapsedGroup" {
        // Mermaid appends the separator and ellipsis after the labelHelper output.
        let geometry = shapes::render_collapsed_group_body(out, ctx, &common, &mut label, details);
        out.checkpoint()?;
        label::render_flowchart_node_label_before_tail(
            out,
            ctx,
            &common,
            &label,
            details,
            div_style_scratch,
        );
        shapes::render_collapsed_group_indicators(out, geometry);
        out.checkpoint()?;
        out.push_str("</g>");
        if common.wrapped_in_a {
            out.push_str("</a>");
        }
    } else if shapes::try_render_flowchart_no_label(out, ctx, &common, details) {
        out.push_str("</g>");
        if common.wrapped_in_a {
            out.push_str("</a>");
        }
    } else if !shapes::render_flowchart_shape(out, ctx, &common, &mut label, details)? {
        label::render_flowchart_node_label(out, ctx, &common, &label, details, div_style_scratch);
    }

    // HandDrawn RoughJS generation may reject before the bounded SVG sink sees its output. Replay
    // that terminal here so a fallback shape cannot silently turn a resource rejection into a
    // successful render.
    ctx.checkpoint_emit()?;
    Ok(())
}

fn prepare_node_geometry<'a>(
    ctx: &'a FlowchartRenderCtx<'a>,
    node_id: &'a str,
    resolved: &helpers::ResolvedNodeRenderInfo<'a>,
) -> merman_render_core::Result<FlowchartNodeGeometry<'a>> {
    let layout_node = ctx.layout_nodes_by_id.get(node_id).ok_or_else(|| {
        merman_render_core::Error::InvalidModel {
            message: format!("missing Flowchart layout node `{node_id}`"),
        }
    })?;
    let prepared = ctx.prepared_nodes.node(node_id).ok_or_else(|| {
        merman_render_core::Error::InvalidModel {
            message: format!("Flowchart node `{node_id}` has no prepared terminal style"),
        }
    })?;
    let selection = &prepared.selection;
    Ok(FlowchartNodeGeometry {
        node_id,
        shape: resolved.shape,
        look: ctx.compatibility.look.as_str(),
        layout_node,
        canonical_axis: ctx.layout.route_contract.is_canonical_axis(),
        node_classes: resolved.node_classes,
        node_styles: resolved.node_styles,
        node_icon: resolved.node_icon,
        node_img: resolved.node_img,
        node_pos: resolved.node_pos,
        node_constraint: resolved.node_constraint,
        node_asset_width: resolved.node_asset_width,
        node_asset_height: resolved.node_asset_height,
        stroke_width: selection.stroke_width,
        stroke_dasharray: prepared.stroke_dasharray(),
        native_corner_radius: selection.native_radius.map(f64::from),
        source_corner_radii: selection.source_radii,
        configured_corner_radius: selection.configured_radius,
        hand_drawn_seed: &ctx.hand_drawn_seed,
        work_meter: ctx.work_meter,
        source_text_style: prepared.source_text_style.as_ref(),
    })
}

pub fn advance_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
) -> merman_render_core::Result<()> {
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(merman_render_core::Error::from)?;
    if !ctx.layout_nodes_by_id.contains_key(node_id) {
        return Ok(());
    }
    let Some(resolved) = helpers::resolve_node_render_info(ctx, node_id) else {
        return Ok(());
    };
    let geometry = prepare_node_geometry(ctx, node_id, &resolved)?;
    let mut label = FlowchartNodeLabelState {
        text: if resolved.label_text_is_node_id {
            node_id
        } else {
            resolved.label_text
        },
        label_type: resolved.label_type,
        dx: 0.0,
        dy: 0.0,
    };
    shapes::advance_rough_geometry(ctx, &geometry, &mut label)?;
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(merman_render_core::Error::from)
}
