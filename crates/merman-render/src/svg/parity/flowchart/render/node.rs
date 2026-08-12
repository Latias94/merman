//! Flowchart node renderer.

use super::super::*;
use crate::svg::parity::timing::RenderTiming;

pub(in crate::svg::parity::flowchart) mod geom;
mod helpers;
mod label;
pub(in crate::svg::parity) mod roughjs;
pub(in crate::svg::parity::flowchart) mod shapes;

pub(in crate::svg::parity::flowchart) use helpers::compute_node_label_metrics;

pub(in crate::svg::parity::flowchart::render) struct FlowchartNodeRenderCommon<'a> {
    pub node_id: &'a str,
    pub shape: &'a str,
    pub look: &'a str,
    pub layout_node: &'a crate::model::LayoutNode,
    pub node_classes: &'a [String],
    pub node_styles: &'a [String],
    pub node_icon: Option<&'a str>,
    pub node_img: Option<&'a str>,
    pub node_pos: Option<&'a str>,
    pub node_constraint: Option<&'a str>,
    pub node_asset_width: Option<f64>,
    pub node_asset_height: Option<f64>,
    pub style: &'a str,
    pub rough_group_style: &'a str,
    pub fill_color: &'a str,
    pub stroke_color: &'a str,
    pub stroke_width: f32,
    pub stroke_dasharray: &'a str,
    pub corner_radius: f64,
    pub hand_drawn_seed: &'a roughr::core::RoughRandomness,
    pub wrapped_in_a: bool,
    pub timing: RenderTiming,
}

impl FlowchartNodeRenderCommon<'_> {
    pub(super) fn look_is_neo(&self) -> bool {
        self.look == "neo"
    }

    pub(super) fn look_is_hand_drawn(&self) -> bool {
        self.look == "handDrawn"
    }
}

fn flowchart_hand_drawn_shape_group_style(inline_styles: &[String]) -> String {
    let mut node_decls: Vec<String> = Vec::new();
    let mut text_decls: Vec<String> = Vec::new();

    for raw in inline_styles {
        for decl in crate::flowchart::flowchart_split_mermaid_style_decls(raw) {
            let Some((key, value)) = crate::mermaid_style::parse_safe_style_decl(decl) else {
                continue;
            };
            if is_text_style_key(key) {
                text_decls.push(format!("{key}:{value}"));
            } else {
                node_decls.push(format!("{key}:{value} !important"));
            }
        }
    }

    if node_decls.is_empty() {
        text_decls.join(";")
    } else {
        node_decls.join(";")
    }
}

pub(in crate::svg::parity::flowchart::render) struct FlowchartNodeLabelState<'a> {
    pub text: &'a str,
    pub label_type: &'a str,
    pub dx: f64,
    pub dy: f64,
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_node(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
    timing: RenderTiming,
    details: &mut FlowchartRenderDetails,
) -> crate::Result<()> {
    let Some(layout_node) = ctx.layout_nodes_by_id.get(node_id) else {
        return Ok(());
    };

    let x = layout_node.x + ctx.tx - origin_x;
    let y = layout_node.y + ctx.ty - origin_y;

    if helpers::try_render_self_loop_label_placeholder(out, node_id, x, y, ctx.node_html_labels) {
        return Ok(());
    }

    let Some(resolved) = helpers::resolve_node_render_info(ctx, node_id) else {
        return Ok(());
    };

    let tooltip = ctx.tooltips.get(node_id).map(|s| s.as_str()).unwrap_or("");
    let tooltip_enabled = !tooltip.trim().is_empty();

    let look = flowchart_config_look(ctx.config);
    let dom_idx = resolved.dom_idx;
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
    let href = resolved.href;
    let target = resolved.target;
    let shape: &str = resolved.shape;
    let node_icon = resolved.node_icon;
    let node_img = resolved.node_img;
    let node_pos = resolved.node_pos;
    let node_constraint = resolved.node_constraint;
    let node_asset_width = resolved.node_asset_width;
    let node_asset_height = resolved.node_asset_height;
    let node_styles = resolved.node_styles;
    let node_classes = resolved.node_classes;

    let empty_classes: &[String] = &[];
    let node_classes_for_wrapper = match shape {
        // Mermaid flowchart-v2 start/stop nodes do not carry classDef classes on the wrapper.
        // Styling is applied via inline styles on the shape paths (stop) or ignored (start).
        "sm-circ" | "small-circle" | "start" | "fr-circ" | "framed-circle" | "stop" => {
            empty_classes
        }
        _ => node_classes,
    };
    helpers::open_node_wrapper(
        out,
        helpers::NodeWrapperAttrs {
            diagram_id: ctx.diagram_id,
            node_id,
            dom_idx,
            class_attr_base,
            node_classes: node_classes_for_wrapper,
            wrapped_in_a,
            href: href.as_ref(),
            target,
            x,
            y,
            tooltip_enabled,
            tooltip,
            look,
        },
    );

    let style_start = timing.start();
    let mut compiled_styles =
        flowchart_compile_node_styles(ctx.class_defs, node_classes, node_styles, &[]);
    if let Some(s) = style_start {
        details.node_style_compile += s.elapsed();
    }
    let source_fill = crate::flowchart::FlowchartSourcePaintStatus::from_parts(
        compiled_styles.fill_declared,
        compiled_styles.fill.is_some(),
    );
    let source_stroke = crate::flowchart::FlowchartSourcePaintStatus::from_parts(
        compiled_styles.stroke_declared,
        compiled_styles.stroke.is_some(),
    );
    let source_paint_residuals = compiled_styles.unverified_shape_paint_residuals(node_id);
    let paint_support = node_theme_paint_support(shape, look)?;
    let node_theme =
        crate::flowchart::FlowchartNodeThemeStyle::resolve(ctx.resolved_theme, ctx.work_meter)?;
    let mut style = std::mem::take(&mut compiled_styles.node_style);
    node_theme.append_inline_paint(
        &mut style,
        source_fill,
        source_stroke,
        paint_support.apply_fill,
        paint_support.apply_stroke,
    );
    let rough_group_style = flowchart_hand_drawn_shape_group_style(node_styles);
    let fill_color = compiled_styles
        .fill
        .as_deref()
        .or_else(|| node_theme.fill_value(source_fill, paint_support.apply_fill))
        .unwrap_or(ctx.node_fill_color.as_str());
    let stroke_color = compiled_styles
        .stroke
        .as_deref()
        .or_else(|| node_theme.stroke_value(source_stroke, paint_support.apply_stroke))
        .unwrap_or(ctx.node_border_color.as_str());
    let stroke_width: f32 = compiled_styles
        .stroke_width
        .as_deref()
        .and_then(|v| v.trim_end_matches("px").trim().parse::<f32>().ok())
        .unwrap_or(1.3);
    let stroke_dasharray = compiled_styles
        .stroke_dasharray
        .as_deref()
        .unwrap_or("0 0")
        .trim();

    let common = FlowchartNodeRenderCommon {
        node_id,
        shape,
        look,
        layout_node,
        node_classes,
        node_styles,
        node_icon,
        node_img,
        node_pos,
        node_constraint,
        node_asset_width,
        node_asset_height,
        style: &style,
        rough_group_style: &rough_group_style,
        fill_color,
        stroke_color,
        stroke_width,
        stroke_dasharray,
        corner_radius: ctx.node_corner_radius,
        hand_drawn_seed: &ctx.hand_drawn_seed,
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

    if shapes::try_render_flowchart_no_label(out, ctx, &common, details) {
        ctx.theme_evidence.record_node_emission(
            &node_theme,
            source_fill,
            source_stroke,
            paint_support.verified_fill,
            paint_support.verified_stroke,
            &source_paint_residuals,
        );
        out.push_str("</g>");
        if common.wrapped_in_a {
            out.push_str("</a>");
        }
        return Ok(());
    }

    let shape_closed_wrapper =
        shapes::render_flowchart_shape(out, ctx, &common, &mut label, details)?;
    ctx.theme_evidence.record_node_emission(
        &node_theme,
        source_fill,
        source_stroke,
        paint_support.verified_fill,
        paint_support.verified_stroke,
        &source_paint_residuals,
    );
    if shape_closed_wrapper {
        return Ok(());
    }

    label::render_flowchart_node_label(out, ctx, &common, &label, &compiled_styles, details);
    Ok(())
}

#[derive(Clone, Copy)]
struct FlowchartNodePaintSupport {
    apply_fill: bool,
    apply_stroke: bool,
    verified_fill: bool,
    verified_stroke: bool,
}

fn node_theme_paint_support(shape: &str, look: &str) -> crate::Result<FlowchartNodePaintSupport> {
    let shape = crate::flowchart::FlowchartShape::resolve(shape)?;
    let verified =
        look != "handDrawn" && matches!(shape, crate::flowchart::FlowchartShape::Process);
    // The typed owner keeps best-effort visual continuity on every Node surface through the
    // existing inline/common-color inputs. Only the standard Process surface currently produces
    // positive portability evidence; all other shapes remain residual until their emitter is
    // verified directly.
    let support = FlowchartNodePaintSupport {
        apply_fill: true,
        apply_stroke: true,
        verified_fill: verified,
        verified_stroke: verified,
    };
    Ok(support)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_node_paint_only_claims_the_verified_standard_svg_surface() {
        let process = node_theme_paint_support("rect", "classic").expect("process shape");
        assert!(process.apply_fill);
        assert!(process.apply_stroke);
        assert!(process.verified_fill);
        assert!(process.verified_stroke);

        for (shape, look) in [
            ("rect", "handDrawn"),
            ("start", "classic"),
            ("anchor", "classic"),
            ("icon", "classic"),
            ("circle", "classic"),
        ] {
            let support = node_theme_paint_support(shape, look).expect("known Flowchart shape");
            assert!(
                support.apply_fill,
                "typed fill was not forwarded for {shape}/{look}"
            );
            assert!(
                support.apply_stroke,
                "typed stroke was not forwarded for {shape}/{look}"
            );
            assert!(
                !support.verified_fill,
                "unexpected verified fill for {shape}/{look}"
            );
            assert!(
                !support.verified_stroke,
                "unexpected verified stroke for {shape}/{look}"
            );
        }
    }
}
