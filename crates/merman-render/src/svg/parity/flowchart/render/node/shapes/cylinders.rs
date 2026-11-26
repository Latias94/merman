//! Flowchart v2 cylinder shapes.

use crate::svg::parity::flowchart::types::{FlowchartRenderCtx, FlowchartRenderDetails};
use crate::svg::parity::{SvgOutput, escape_attr, fmt, fmt_display};

use super::super::{FlowchartNodeLabelState, FlowchartNodeRenderCommon, helpers};

pub(super) struct CylinderGeometry {
    pub(super) w: f64,
    pub(super) h: f64,
    pub(super) rx: f64,
    pub(super) ry: f64,
}

impl CylinderGeometry {
    pub(super) fn vertical(common: &FlowchartNodeRenderCommon<'_>, min_body_height: f64) -> Self {
        let w = common.layout_node.width.max(1.0);
        let rx = w / 2.0;
        let ry = rx / (2.5 + w / 50.0);
        let h = (common.layout_node.height.max(1.0) - 2.0 * ry).max(min_body_height);
        Self { w, h, rx, ry }
    }

    fn horizontal(
        ctx: &FlowchartRenderCtx<'_>,
        common: &FlowchartNodeRenderCommon<'_>,
        label: &FlowchartNodeLabelState<'_>,
    ) -> Self {
        // Mermaid lays out the tilted cylinder's painted path from the label bbox.
        let metrics = helpers::compute_node_label_metrics(
            ctx,
            common.node_id,
            Some(common.layout_node),
            label.text,
            label.label_type,
            common.node_classes,
            common.node_styles,
        );
        let label_padding = if common.look_is_neo() {
            12.0
        } else {
            ctx.node_padding / 2.0
        };
        let h = (metrics.height + label_padding).max(1.0);
        let ry = h / 2.0;
        let rx = ry / (2.5 + h / 50.0);
        let w = (metrics.width + rx + label_padding).max(1.0);
        Self { w, h, rx, ry }
    }

    fn vertical_body(&self) -> String {
        format!(
            "M0,{ry} a{rx},{ry} 0,0,1 {w},0 v{h} a{rx},{ry} 0,0,1 {mw},0 Z",
            rx = fmt(self.rx),
            ry = fmt(self.ry),
            w = fmt(self.w),
            h = fmt(self.h),
            mw = fmt(-self.w),
        )
    }

    fn vertical_rim(&self, y: f64) -> String {
        format!(
            "M0,{} a{},{} 0,0,0 {},0",
            fmt(y),
            fmt(self.rx),
            fmt(self.ry),
            fmt(self.w),
        )
    }

    fn horizontal_body(&self) -> String {
        format!(
            "M0,0 a{rx},{ry} 0,0,1 0,{mh} h{w} a{rx},{ry} 0,0,1 0,{h} Z",
            rx = fmt(self.rx),
            ry = fmt(self.ry),
            w = fmt(self.w),
            h = fmt(self.h),
            mh = fmt(-self.h),
        )
    }
}

fn uses_hand_drawn_cylinder(
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
) -> bool {
    // Shoal emits one root with its edges before nodes. Rough cylinders painted here cannot
    // affect later edges' randomness in the geometry query.
    common.look_is_hand_drawn() && ctx.layout.route_contract.is_canonical_axis()
}

pub(super) fn render_hand_drawn_vertical_cylinder(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    details: &mut FlowchartRenderDetails,
    geometry: &CylinderGeometry,
    second_rim: Option<f64>,
) -> bool {
    if !uses_hand_drawn_cylinder(ctx, common) {
        return false;
    }
    let mut rims = geometry.vertical_rim(geometry.ry);
    if let Some(y) = second_rim {
        rims.push(' ');
        rims.push_str(&geometry.vertical_rim(y));
    }
    write_hand_drawn_cylinder(
        out,
        common,
        details,
        &geometry.vertical_body(),
        &rims,
        (-geometry.w / 2.0, -(geometry.h / 2.0 + geometry.ry)),
        ctx.security_level_loose.then_some(geometry.ry),
    );
    true
}

fn write_hand_drawn_cylinder(
    out: &mut impl SvgOutput,
    common: &FlowchartNodeRenderCommon<'_>,
    details: &mut FlowchartRenderDetails,
    body: &str,
    rims: &str,
    (tx, ty): (f64, f64),
    label_offset: Option<f64>,
) {
    let _ = write!(
        out,
        r#"<g class="basic label-container outer-path" transform="translate({},{})" style="{}""#,
        fmt(tx),
        fmt(ty),
        escape_attr(common.rough_group_style),
    );
    if let Some(offset) = label_offset {
        let _ = write!(out, r#" label-offset-y="{}""#, fmt(offset));
    }
    out.push('>');
    if let Some((fill, stroke)) = helpers::hand_drawn_path_pair(common, details, body) {
        helpers::write_hand_drawn_path_pair(out, common, &fill, &stroke);
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}" stroke-dasharray="{}"/>"#,
            escape_attr(body),
            escape_attr(common.fill_color),
            escape_attr(common.stroke_color),
            fmt_display(common.stroke_width as f64),
            escape_attr(common.stroke_dasharray),
        );
    }
    // The interior rims are open curves. Feeding them to the hatch generator would fill each cap.
    let _ = write!(
        out,
        r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-dasharray="{}"/></g>"#,
        escape_attr(rims),
        escape_attr(common.stroke_color),
        fmt_display(common.stroke_width as f64),
        escape_attr(common.stroke_dasharray),
    );
}

pub(in crate::svg::parity::flowchart::render::node) fn render_cylinder(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    // Mermaid applies an extra downward label shift of `node.padding / 1.5`.
    label.dy = ctx.node_padding / 1.5;
    let geometry = CylinderGeometry::vertical(common, 1.0);
    if render_hand_drawn_vertical_cylinder(out, ctx, common, details, &geometry, None) {
        return;
    }

    // Mermaid `cylinder.ts`: a single path and, in loose mode, a label offset.
    let CylinderGeometry { w, h, rx, ry } = geometry;
    let path_data = format!(
        "M0,{ry} a{rx},{ry} 0,0,0 {w},0 a{rx},{ry} 0,0,0 {mw},0 l0,{h} a{rx},{ry} 0,0,0 {w},0 l0,{mh}",
        ry = fmt(ry),
        rx = fmt(rx),
        w = fmt(w),
        mw = fmt(-w),
        h = fmt(h),
        mh = fmt(-h),
    );

    let _ = write!(
        out,
        r#"<path d="{}" class="basic label-container outer-path" style="{}""#,
        escape_attr(&path_data),
        escape_attr(common.style),
    );
    if ctx.security_level_loose {
        let _ = write!(out, r#" label-offset-y="{}""#, fmt(ry));
    }
    let _ = write!(
        out,
        r#" transform="translate({},{})"/>"#,
        fmt(-w / 2.0),
        fmt(-(h / 2.0 + ry))
    );
}

pub(in crate::svg::parity::flowchart::render::node) fn render_horizontal_cylinder(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    let geometry = CylinderGeometry::horizontal(ctx, common, label);
    let CylinderGeometry { w, h, rx, ry } = geometry;
    let translation = (-w / 2.0, h / 2.0);
    // Mermaid offsets the label left by `rx` for tilted cylinders.
    label.dx = -rx;
    if !ctx.node_html_labels {
        let node_text_style = helpers::node_source_text_style(
            ctx,
            Some(common.node_id),
            common.node_classes,
            common.node_styles,
        );
        label.dy -= ctx
            .measurer
            .measure_svg_create_text_bbox_y_offset_px(label.text, &node_text_style);
    }

    if uses_hand_drawn_cylinder(ctx, common) {
        let rim = format!(
            "M{},{} a{},{} 0,0,0 0,{}",
            fmt(w),
            fmt(-h),
            fmt(rx),
            fmt(ry),
            fmt(h),
        );
        write_hand_drawn_cylinder(
            out,
            common,
            details,
            &geometry.horizontal_body(),
            &rim,
            translation,
            None,
        );
        return;
    }

    let path_data = format!(
        "M0,0 a{rx},{ry} 0,0,1 0,{neg_h} l{w},0 a{rx},{ry} 0,0,1 0,{h} M{w},{neg_h} a{rx},{ry} 0,0,0 0,{h} l{neg_w},0",
        rx = fmt(rx),
        ry = fmt(ry),
        neg_h = fmt(-h),
        w = fmt(w),
        h = fmt(h),
        neg_w = fmt(-w),
    );

    let _ = write!(
        out,
        r#"<path d="{}" class="basic label-container outer-path" style="{}" transform="translate({},{})"/>"#,
        escape_attr(&path_data),
        escape_attr(common.style),
        fmt(translation.0),
        fmt(translation.1),
    );
}
