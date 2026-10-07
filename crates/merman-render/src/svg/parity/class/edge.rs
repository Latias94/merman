use super::super::timing::RenderTiming;
use super::ClassSvgRelation;
use super::bounds::{
    include_class_marker_paint_bounds, include_path_bounds_with_outset, include_path_d_with_outset,
    include_xywh,
};
use super::context::{ClassEmitCheckpoint, ClassRenderDetails};
use super::defs::{class_marker_name, class_marker_paint_spec};
use super::label::{
    ClassHtmlLabelSpec, class_html_div_style, class_math_html_label, render_class_html_label,
    write_class_svg_edge_text,
};
use super::rough::class_rough_hand_drawn_stroke_path_for_svg_path;
use crate::entities::decode_entities_minimal_cow;
use crate::model::{Bounds, LayoutEdge, LayoutLabel, LayoutPoint};
use crate::svg::parity::SvgDiagramId;
use crate::svg::parity::edge_label_geometry::position_edge_label;
use crate::text::{MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, TextMeasurer, TextStyle};
use base64::Engine as _;
use std::fmt::Write as _;

use super::super::{
    SvgOutput, escape_attr_display, escape_xml_into, fmt, json_stringify_points_into,
};
use rustc_hash::FxHashMap;

const CLASS_HAND_DRAWN_EDGE_STROKE: &str = "#000";
const CLASS_HAND_DRAWN_EDGE_STROKE_WIDTH: &str = "1";

pub(super) struct ClassEdgeGroupsRenderContext<'a> {
    pub edges: &'a [LayoutEdge],
    pub missing_section_points: &'a FxHashMap<&'a str, Vec<LayoutPoint>>,
    pub work_meter: &'a crate::resources::OperationWorkMeter,
    pub line_hop_paths: &'a std::collections::HashMap<&'a str, String>,
    pub relations_by_id: &'a FxHashMap<&'a str, &'a ClassSvgRelation>,
    pub relation_index_by_id: &'a FxHashMap<&'a str, usize>,
    pub diagram_marker_class: &'a str,
    pub diagram_id: SvgDiagramId<'a>,
    pub content_tx: f64,
    pub content_ty: f64,
    pub bounds_dx: f64,
    pub bounds_dy: f64,
    pub edge_use_html_labels: bool,
    pub text_measurer: &'a dyn TextMeasurer,
    pub terminal_text_style: &'a TextStyle,
    pub mermaid_config: Option<&'a merman_core::MermaidConfig>,
    pub math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
    pub look: &'a str,
    pub hand_drawn_seed: roughr::core::RoughRandomness,
    pub timing: RenderTiming,
    pub uses_elk_adapter_dom: bool,
    pub edge_paths_class: &'static str,
    pub text_paint: Option<&'a crate::class::ClassTextPaint>,
    pub relation_theme: &'a crate::class::ClassRelationThemePlan,
    pub emit: ClassEmitCheckpoint<'a>,
}

pub(super) use crate::class::class_arrow_type_for_relation_end;
pub(super) type ClassEdgeLabelCenters = FxHashMap<String, LayoutPoint>;

pub(super) fn class_line_with_marker_offset_points_into(
    input: &[LayoutPoint],
    relation: Option<&ClassSvgRelation>,
    out: &mut Vec<LayoutPoint>,
) {
    fn marker_offset_for(arrow_type: Option<&str>) -> Option<f64> {
        match arrow_type {
            Some("dependency") => Some(6.0),
            Some("lollipop") => Some(13.5),
            Some("aggregation" | "extension" | "composition") => Some(17.25),
            _ => None,
        }
    }

    fn calculate_delta_and_angle(a: &LayoutPoint, b: &LayoutPoint) -> (f64, f64, f64) {
        let delta_x = b.x - a.x;
        let delta_y = b.y - a.y;
        let angle = (delta_y / delta_x).atan();
        (angle, delta_x, delta_y)
    }

    out.clear();
    out.reserve(input.len());
    if input.len() < 2 {
        out.extend(input.iter().cloned());
        return;
    }

    let arrow_type_start =
        relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type1));
    let arrow_type_end =
        relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type2));
    let start = &input[0];
    let end = &input[input.len() - 1];
    let x_direction_is_left = start.x < end.x;
    let y_direction_is_down = start.y < end.y;
    let extra_room = 1.0;
    let start_marker_height = marker_offset_for(arrow_type_start);
    let end_marker_height = marker_offset_for(arrow_type_end);

    for (idx, point) in input.iter().enumerate() {
        let mut offset_x = 0.0;
        let mut offset_y = 0.0;

        if idx == 0 {
            if let Some(height) = start_marker_height {
                let (angle, delta_x, delta_y) = calculate_delta_and_angle(&input[0], &input[1]);
                offset_x = height * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
                offset_y = height * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
            }
        } else if idx == input.len() - 1
            && let Some(height) = end_marker_height
        {
            let (angle, delta_x, delta_y) =
                calculate_delta_and_angle(&input[input.len() - 1], &input[input.len() - 2]);
            offset_x = height * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
            offset_y = height * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
        }

        if let Some(height) = end_marker_height {
            let diff_x = (point.x - end.x).abs();
            let diff_y = (point.y - end.y).abs();
            if diff_x < height && diff_x > 0.0 && diff_y < height {
                let mut adjustment = height + extra_room - diff_x;
                adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
                offset_x -= adjustment;
            }
        }
        if let Some(height) = start_marker_height {
            let diff_x = (point.x - start.x).abs();
            let diff_y = (point.y - start.y).abs();
            if diff_x < height && diff_x > 0.0 && diff_y < height {
                let mut adjustment = height + extra_room - diff_x;
                adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
                offset_x += adjustment;
            }
        }

        if let Some(height) = end_marker_height {
            let diff_y = (point.y - end.y).abs();
            let diff_x = (point.x - end.x).abs();
            if diff_y < height && diff_y > 0.0 && diff_x < height {
                let mut adjustment = height + extra_room - diff_y;
                adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
                offset_y -= adjustment;
            }
        }
        if let Some(height) = start_marker_height {
            let diff_y = (point.y - start.y).abs();
            let diff_x = (point.x - start.x).abs();
            if diff_y < height && diff_y > 0.0 && diff_x < height {
                let mut adjustment = height + extra_room - diff_y;
                adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
                offset_y += adjustment;
            }
        }

        out.push(LayoutPoint {
            x: point.x + offset_x,
            y: point.y + offset_y,
        });
    }
}

pub(super) fn render_class_edge_paths<O: SvgOutput>(
    out: &mut O,
    content_bounds: &mut Option<Bounds>,
    detail: &mut ClassRenderDetails,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
    ctx: &ClassEdgeGroupsRenderContext<'_>,
) -> crate::Result<ClassEdgeLabelCenters> {
    let mut edge_points_json_buf = String::new();
    let mut edge_points_json_ryu = ryu_js::Buffer::new();
    let mut edge_points_b64_buf = String::new();
    let mut edge_raw_points: Vec<LayoutPoint> = Vec::new();
    let mut edge_marker_points: Vec<LayoutPoint> = Vec::new();
    let mut edge_curve_points: Vec<LayoutPoint> = Vec::new();
    let mut edge_class_buf = String::with_capacity(64);
    let mut edge_dom_id_buf = String::with_capacity(64);
    let mut edge_style_buf = String::new();

    let edge_paths_start = ctx.timing.start();
    let ordered_edges = class_edge_render_order(ctx.edges, ctx.relation_index_by_id);
    let mut edge_label_centers =
        ClassEdgeLabelCenters::with_capacity_and_hasher(ordered_edges.len(), Default::default());
    let _ = write!(out, r#"<g class="{}">"#, ctx.edge_paths_class);
    out.checkpoint()?;
    for e in ordered_edges.iter().copied() {
        ctx.emit.checkpoint()?;
        let missing_section = ctx.missing_section_points.get(e.id.as_str());
        let source_points = missing_section.map(Vec::as_slice).unwrap_or(&e.points);
        if source_points.is_empty() || (missing_section.is_none() && source_points.len() < 2) {
            continue;
        }

        class_edge_dom_id_into(&mut edge_dom_id_buf, e, ctx.relation_index_by_id);

        edge_raw_points.clear();
        edge_raw_points.reserve(source_points.len());
        for p in source_points {
            edge_raw_points.push(LayoutPoint {
                x: p.x + ctx.content_tx,
                y: p.y + ctx.content_ty,
            });
        }

        let curve_start = ctx.timing.start();
        let is_note_edge = e.id.starts_with("edgeNote");
        let relation = if is_note_edge {
            None
        } else {
            ctx.relations_by_id.get(e.id.as_str()).copied()
        };
        let relation_stroke_width = relation.and(ctx.relation_theme.paint_stroke_width());
        let typed_stroke = if relation.is_some() || is_note_edge {
            ctx.relation_theme.typed_stroke()
        } else {
            None
        };
        let typed_stroke_width = relation.and(ctx.relation_theme.typed_stroke_width());
        let stroke_outset = relation_stroke_width.map_or(0.0, |width| f64::from(width) / 2.0);
        let start_marker_name =
            relation.and_then(|relation| class_marker_name(relation.relation.type1, true));
        let end_marker_name =
            relation.and_then(|relation| class_marker_name(relation.relation.type2, false));
        let start_marker_paint = relation
            .and_then(|rel| class_marker_paint_spec(rel.relation.type1, true, ctx.look == "neo"));
        let end_marker_paint = relation
            .and_then(|rel| class_marker_paint_spec(rel.relation.type2, false, ctx.look == "neo"));
        if ctx.uses_elk_adapter_dom && missing_section.is_none() {
            // ELK selects rounded routing after clipping and offsets only the endpoints.
            edge_marker_points = super::super::edge_path::rounded_line_with_marker_offsets_points(
                &edge_raw_points,
                relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type1)),
                relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type2)),
            );
        } else {
            class_line_with_marker_offset_points_into(
                &edge_raw_points,
                relation,
                &mut edge_marker_points,
            );
        }
        let edge_curve_source = edge_marker_points.as_slice();
        let (d, d_pb) = if missing_section.is_some() {
            super::super::curve::curve_linear_path_d_and_bounds(edge_curve_source)
        } else if ctx.uses_elk_adapter_dom {
            super::super::curve::curve_rounded_path_d_and_bounds(
                edge_curve_source,
                5.0,
                false,
                None,
            )
        } else if edge_curve_source.len() == 2 {
            edge_curve_points.clear();
            let a = &edge_curve_source[0];
            let b = &edge_curve_source[1];
            edge_curve_points.push(a.clone());
            edge_curve_points.push(LayoutPoint {
                x: (a.x + b.x) / 2.0,
                y: (a.y + b.y) / 2.0,
            });
            edge_curve_points.push(b.clone());
            super::super::curve::curve_basis_path_d_and_bounds(&edge_curve_points)
        } else {
            super::super::curve::curve_basis_path_d_and_bounds(edge_curve_source)
        };
        if let Some(s) = curve_start {
            detail.edge_curve += s.elapsed();
        }
        let rough_d = if ctx.look == "handDrawn" {
            class_rough_hand_drawn_stroke_path_for_svg_path(&d, 0.3, &ctx.hand_drawn_seed)
        } else {
            None
        };
        let paint_d = rough_d.as_deref().unwrap_or(&d);
        let hopped_d = ctx.line_hop_paths.get(e.id.as_str());
        let render_d = hopped_d.map(String::as_str).unwrap_or(paint_d);
        if let Some(lbl) = e.label.as_ref() {
            edge_label_centers.insert(
                e.id.to_string(),
                if missing_section.is_some() {
                    let first = &edge_raw_points[0];
                    let last = &edge_raw_points[edge_raw_points.len() - 1];
                    LayoutPoint {
                        x: (first.x + last.x) / 2.0,
                        y: (first.y + last.y) / 2.0,
                    }
                } else {
                    class_edge_label_center(
                        Some(&edge_raw_points),
                        &edge_raw_points,
                        paint_d,
                        e.from_cluster.is_some() || e.to_cluster.is_some(),
                        lbl,
                        ctx.content_tx,
                        ctx.content_ty,
                    )
                },
            );
        }
        let path_bounds_start = ctx.timing.start();
        // RoughJS replaces the stable curve endpoints with jittered path data. Use the rendered
        // path bbox plus a rotation-independent marker radius to cover its actual first/final
        // marker anchors without maintaining a second SVG path interpreter. Classic and ELK paths
        // retain one stable endpoint pair and can use the tighter rotated marker boxes below.
        let rough_marker_outset = if rough_d.is_some() {
            relation_stroke_width.map_or(0.0, |width| {
                [start_marker_paint, end_marker_paint]
                    .into_iter()
                    .flatten()
                    .map(|marker| marker.conservative_radius(f64::from(width)))
                    .fold(0.0, f64::max)
            })
        } else {
            0.0
        };
        let path_paint_outset = stroke_outset.max(rough_marker_outset);
        if rough_d.is_none()
            && hopped_d.is_none()
            && let Some(pb) = d_pb.as_ref()
        {
            include_path_bounds_with_outset(
                content_bounds,
                pb,
                ctx.bounds_dx,
                ctx.bounds_dy,
                path_paint_outset,
            );
        } else {
            include_path_d_with_outset(
                content_bounds,
                render_d,
                ctx.bounds_dx,
                ctx.bounds_dy,
                path_paint_outset,
            );
        }
        if rough_d.is_none()
            && let Some(width) = relation_stroke_width.map(f64::from)
        {
            if let Some(marker) = start_marker_paint {
                include_class_marker_paint_bounds(
                    content_bounds,
                    edge_curve_source,
                    true,
                    marker,
                    width,
                    ctx.bounds_dx,
                    ctx.bounds_dy,
                );
            }
            if let Some(marker) = end_marker_paint {
                include_class_marker_paint_bounds(
                    content_bounds,
                    edge_curve_source,
                    false,
                    marker,
                    width,
                    ctx.bounds_dx,
                    ctx.bounds_dy,
                );
            }
        }
        if let Some(s) = path_bounds_start {
            detail.path_bounds += s.elapsed();
            detail.path_bounds_calls += 1;
        }

        let json_start = ctx.timing.start();
        edge_points_json_buf.clear();
        json_stringify_points_into(
            &mut edge_points_json_buf,
            &edge_raw_points,
            &mut edge_points_json_ryu,
        );
        if let Some(s) = json_start {
            detail.edge_points_json += s.elapsed();
        }

        let b64_start = ctx.timing.start();
        edge_points_b64_buf.clear();
        base64::engine::general_purpose::STANDARD
            .encode_string(edge_points_json_buf.as_bytes(), &mut edge_points_b64_buf);
        if let Some(s) = b64_start {
            detail.edge_points_b64 += s.elapsed();
        }

        edge_class_buf.clear();
        edge_class_buf.push_str("edge-thickness-normal ");
        if is_note_edge {
            edge_class_buf.push_str(class_note_edge_pattern());
        } else if let Some(rel) = relation {
            edge_class_buf.push_str(class_edge_pattern(rel.relation.line_type));
        } else {
            edge_class_buf.push_str("edge-pattern-solid");
        }
        if ctx.look == "handDrawn" {
            edge_class_buf.push_str(" transition");
        }
        edge_class_buf.push_str(" relation");

        let _ = write!(out, r#"<path d="{}""#, escape_attr_display(render_d));
        let hand_drawn_stroke = (ctx.look == "handDrawn").then(|| {
            typed_stroke
                .map(|(_, css)| css)
                .unwrap_or(CLASS_HAND_DRAWN_EDGE_STROKE)
        });
        if ctx.look == "handDrawn" {
            let _ = write!(
                out,
                r#" stroke="{}" stroke-width="{}" fill="none""#,
                escape_attr_display(hand_drawn_stroke.unwrap_or(CLASS_HAND_DRAWN_EDGE_STROKE)),
                CLASS_HAND_DRAWN_EDGE_STROKE_WIDTH,
            );
        }
        out.push_str(r#" id=""#);
        let _ = write!(out, "{}", ctx.diagram_id);
        ctx.emit.checkpoint()?;
        let _ = write!(
            out,
            r#"-{}" class="{}" data-edge="true" data-et="edge" data-id="{}" data-points="{}""#,
            escape_attr_display(&edge_dom_id_buf),
            escape_attr_display(&edge_class_buf),
            escape_attr_display(&edge_dom_id_buf),
            escape_attr_display(&edge_points_b64_buf),
        );
        let _ = write!(out, r#" data-look="{}""#, escape_attr_display(ctx.look));
        if let Some(name) = start_marker_name {
            out.push_str(r#" marker-start="url(#"#);
            let _ = write!(out, "{}", ctx.diagram_id);
            ctx.emit.checkpoint()?;
            let _ = write!(
                out,
                r#"_{}-{}{})""#,
                escape_attr_display(ctx.diagram_marker_class),
                name,
                if ctx.look == "neo" { "-margin" } else { "" },
            );
        }
        if let Some(name) = end_marker_name {
            out.push_str(r#" marker-end="url(#"#);
            let _ = write!(out, "{}", ctx.diagram_id);
            ctx.emit.checkpoint()?;
            let _ = write!(
                out,
                r#"_{}-{}{})""#,
                escape_attr_display(ctx.diagram_marker_class),
                name,
                if ctx.look == "neo" { "-margin" } else { "" },
            );
        }
        edge_style_buf.clear();
        if ctx.look == "neo"
            && let Some(length) = super::super::svg_path_length_from_d(paint_d)
        {
            super::super::edge_path::write_neo_edge_mask(
                &mut edge_style_buf,
                length,
                relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type1)),
                relation.and_then(|rel| class_arrow_type_for_relation_end(rel.relation.type2)),
                edge_class_buf
                    .split_whitespace()
                    .any(|class| class == "edge-pattern-dashed"),
                false,
            );
        }
        edge_style_buf.push_str(class_edge_path_style(
            e.id.as_str(),
            ctx.look == "handDrawn",
        ));
        let mut terminal_style = if let Some(hopped_d) = hopped_d {
            super::super::line_hops::rewrite_style_after_line_hop(
                &edge_style_buf,
                hopped_d,
                ctx.work_meter,
            )?
            .into_owned()
        } else {
            edge_style_buf.clone()
        };
        if let Some((_, stroke)) = typed_stroke {
            if !terminal_style.ends_with(';') {
                terminal_style.push(';');
            }
            let _ = write!(&mut terminal_style, "stroke:{stroke} !important;");
        }
        if let Some(width) = typed_stroke_width {
            let _ = write!(
                &mut terminal_style,
                "stroke-width:{}px !important",
                fmt(f64::from(width)),
            );
        }
        let _ = write!(out, r#" style="{}""#, escape_attr_display(&terminal_style));
        out.push_str("/>");
        out.checkpoint()?;
        if is_note_edge {
            let note_index =
                e.id.strip_prefix("edgeNote")
                    .and_then(|index| index.parse::<usize>().ok())
                    .unwrap_or(usize::MAX);
            theme_receipt.record_note_attachment(
                note_index,
                typed_stroke,
                &terminal_style,
                hand_drawn_stroke,
            );
        } else if relation.is_some() {
            let relation_index = ctx
                .relation_index_by_id
                .get(e.id.as_str())
                .copied()
                .and_then(|index| index.checked_sub(1))
                .unwrap_or(usize::MAX);
            theme_receipt.record_relation(
                relation_index,
                start_marker_name,
                end_marker_name,
                typed_stroke,
                &terminal_style,
                hand_drawn_stroke,
                typed_stroke_width.is_some(),
            );
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;
    if let Some(s) = edge_paths_start {
        detail.edge_paths += s.elapsed();
    }

    Ok(edge_label_centers)
}

pub(super) fn render_class_edge_labels<O: SvgOutput>(
    out: &mut O,
    content_bounds: &mut Option<Bounds>,
    detail: &mut ClassRenderDetails,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
    typography_receipt: &mut Option<crate::class::ClassTextThemeReceipt>,
    ctx: &ClassEdgeGroupsRenderContext<'_>,
    edge_label_centers: &ClassEdgeLabelCenters,
) -> crate::Result<()> {
    let mut edge_dom_id_buf = String::with_capacity(64);
    let edge_labels_start = ctx.timing.start();
    let ordered_edges = class_edge_render_order(ctx.edges, ctx.relation_index_by_id);
    out.push_str(r#"<g class="edgeLabels">"#);
    out.checkpoint()?;
    // Mermaid's serialized SVG keeps all `edgeLabel` groups before `edgeTerminals`.
    // ELK awaits each edge label and its terminals; Dagre inserts center labels concurrently.
    for e in ordered_edges.iter().copied() {
        ctx.emit.checkpoint()?;
        class_edge_dom_id_into(&mut edge_dom_id_buf, e, ctx.relation_index_by_id);
        let relation = ctx.relations_by_id.get(e.id.as_str()).copied();
        let label_text = relation
            .map(|relation| relation.title.as_str())
            .unwrap_or("");

        // The common registered-layout renderer only inserts labels for edges with
        // center or terminal text. Dagre still inserts an empty center wrapper.
        let has_terminal_label = ctx.relations_by_id.get(e.id.as_str()).is_some_and(|rel| {
            [&rel.relation_title_1, &rel.relation_title_2]
                .into_iter()
                .flatten()
                .any(|text| !text.is_empty() && text != "none")
        });
        if ctx.uses_elk_adapter_dom && label_text.is_empty() && !has_terminal_label {
            if let Some(&index) = ctx.relation_index_by_id.get(e.id.as_str()) {
                theme_receipt.record_edge_label_background(
                    index.checked_sub(1).unwrap_or(usize::MAX),
                    false,
                );
            }
            if relation.is_some()
                && let Some(receipt) = typography_receipt.as_mut()
            {
                // ELK omits every label DOM terminal on this route. Record that observed
                // absence so the receipt can distinguish it from an unvisited writer.
                receipt.record_edge_label(e.id.as_str(), Default::default());
                for slot in 0..4 {
                    receipt.record_cardinality(e.id.as_str(), slot, Default::default());
                }
            }
            continue;
        }

        let label_center = e.label.as_ref().map(|lbl| {
            edge_label_centers
                .get(e.id.as_str())
                .cloned()
                .unwrap_or(LayoutPoint {
                    x: lbl.x + ctx.content_tx,
                    y: lbl.y + ctx.content_ty,
                })
        });
        if !label_text.trim().is_empty()
            && let (Some(lbl), Some(center)) = (e.label.as_ref(), label_center.as_ref())
        {
            include_xywh(
                content_bounds,
                center.x - lbl.width / 2.0 + ctx.bounds_dx,
                center.y - lbl.height / 2.0 + ctx.bounds_dy,
                lbl.width.max(0.0),
                lbl.height.max(0.0),
            );
        }
        let (typography, background_visible) = render_class_edge_label_group(
            out,
            edge_dom_id_buf.as_str(),
            label_text,
            e.label.as_ref(),
            label_center.as_ref().map(|center| center.x).unwrap_or(0.0),
            label_center.as_ref().map(|center| center.y).unwrap_or(0.0),
            ctx,
            theme_receipt,
            ctx.relation_index_by_id
                .get(e.id.as_str())
                .copied()
                .and_then(|index| index.checked_sub(1)),
        );
        out.checkpoint()?;
        if let Some(&index) = ctx.relation_index_by_id.get(e.id.as_str()) {
            theme_receipt.record_edge_label_background(
                index.checked_sub(1).unwrap_or(usize::MAX),
                background_visible,
            );
        }
        if relation.is_some()
            && let Some(receipt) = typography_receipt.as_mut()
        {
            receipt.record_edge_label(e.id.as_str(), typography);
        }
        if ctx.uses_elk_adapter_dom {
            render_class_edge_terminals(out, e, true, content_bounds, ctx, typography_receipt)?;
            render_class_edge_terminals(out, e, false, content_bounds, ctx, typography_receipt)?;
        }
    }
    if !ctx.uses_elk_adapter_dom {
        for e in ordered_edges.iter().copied() {
            ctx.emit.checkpoint()?;
            render_class_edge_terminals(out, e, true, content_bounds, ctx, typography_receipt)?;
        }
        // Dagre starts all insertEdgeLabel futures together. End-only labels precede
        // labels whose start terminal adds another await before their end terminal.
        let mut ordered_end_edges = ordered_edges
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();
        ordered_end_edges.sort_by_key(|(idx, edge)| {
            (
                edge.start_label_left.is_some() || edge.start_label_right.is_some(),
                *idx,
            )
        });
        for (_, e) in ordered_end_edges {
            ctx.emit.checkpoint()?;
            render_class_edge_terminals(out, e, false, content_bounds, ctx, typography_receipt)?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;
    if let Some(s) = edge_labels_start {
        detail.edge_labels += s.elapsed();
    }
    Ok(())
}

pub(super) fn class_edge_label_center(
    original_path_points: Option<&[LayoutPoint]>,
    label_path_points: &[LayoutPoint],
    rendered_d: &str,
    points_were_explicitly_updated: bool,
    label: &LayoutLabel,
    content_tx: f64,
    content_ty: f64,
) -> LayoutPoint {
    position_edge_label(
        LayoutPoint {
            x: label.x + content_tx,
            y: label.y + content_ty,
        },
        original_path_points,
        label_path_points,
        rendered_d,
        points_were_explicitly_updated,
    )
}

fn render_class_edge_label_group(
    out: &mut impl SvgOutput,
    dom_id: &str,
    label_text: &str,
    label: Option<&LayoutLabel>,
    center_x: f64,
    center_y: f64,
    ctx: &ClassEdgeGroupsRenderContext<'_>,
    receipt: &mut crate::class::ClassRelationThemeReceipt,
    relation_index: Option<usize>,
) -> (crate::class::ClassTextTerminalFacts, bool) {
    let normalized = if ctx.uses_elk_adapter_dom {
        crate::text::mermaid_html_breaks_to_newlines(label_text)
    } else {
        std::borrow::Cow::Borrowed(label_text)
    };
    let decoded = decode_entities_minimal_cow(&normalized);
    let trimmed = decoded.trim();
    let background_visible =
        !trimmed.is_empty() && label.is_some_and(|label| label.width > 0.0 && label.height > 0.0);
    let use_html_labels = ctx.edge_use_html_labels || crate::math::contains_delimited_math(trimmed);
    let mut emitted_style = None;
    let background = ctx.relation_theme.edge_label_background();
    let html_background_style =
        background.map(|(_, css)| format!("background-color:{css} !important;"));
    let span_style = html_background_style.as_ref().map(|background| {
        format!(
            "{}{background}",
            ctx.text_paint.map(|paint| paint.style()).unwrap_or("")
        )
    });
    if use_html_labels {
        let empty_div_style =
            class_html_div_style(0.0, MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX as i64);
        if trimmed.is_empty() {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr_display(dom_id),
                escape_attr_display(empty_div_style.as_str())
            );
        } else if let Some(lbl) = label {
            let mut div_style = class_html_div_style(
                lbl.width.max(0.0),
                MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX as i64,
            );
            if let Some(style) = &html_background_style {
                div_style.push_str(style);
            }
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}">"#,
                fmt(center_x),
                fmt(center_y),
                escape_attr_display(dom_id),
                fmt(-lbl.width / 2.0),
                fmt(-lbl.height / 2.0),
                fmt(lbl.width.max(0.0)),
                fmt(lbl.height.max(0.0)),
                escape_attr_display(div_style.as_str()),
            );
            emitted_style = render_class_html_label(
                out,
                &ClassHtmlLabelSpec {
                    span_class: "edgeLabel",
                    text: trimmed,
                    include_p: true,
                    extra_span_class: None,
                    span_style: span_style
                        .as_deref()
                        .or_else(|| ctx.text_paint.map(|paint| paint.style())),
                    prepared_xhtml: None,
                    mermaid_config: ctx.mermaid_config,
                    math_renderer: ctx.math_renderer,
                },
            );
            out.push_str("</div></foreignObject></g></g>");
            if let Some(index) = relation_index.filter(|_| background_visible) {
                receipt.record_edge_label_background_paint(
                    index,
                    background.map(|(rule, _)| rule),
                    &[
                        (&div_style, "background-color"),
                        (emitted_style.unwrap_or(""), "background-color"),
                    ],
                );
            }
        } else {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr_display(dom_id),
                escape_attr_display(empty_div_style.as_str())
            );
        }
        let typography = if trimmed.is_empty() || label.is_none() {
            crate::class::ClassTextTerminalFacts::default()
        } else if crate::math::contains_delimited_math(trimmed) {
            crate::class::ClassTextTerminalFacts::unverified_text(trimmed)
        } else {
            let facts = crate::class::class_html_label_visible_style_facts(trimmed);
            crate::class::ClassTextTerminalFacts::from_visible_style_facts(&facts)
        };
        let typography = ctx
            .text_paint
            .map_or(typography, |paint| paint.observe(typography, emitted_style));
        return (typography, background_visible);
    }

    if trimmed.is_empty() {
        out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
        let _ = write!(
            out,
            r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)">"#,
            escape_attr_display(dom_id)
        );
        write_class_svg_edge_text(out, "", false);
        out.push_str("</g></g>");
    } else if let Some(lbl) = label {
        let background_style = background
            .map(|(_, css)| format!("fill:{css} !important;"))
            .unwrap_or_default();
        let _ = write!(
            out,
            r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><g><rect class="background" style="{}" x="-2" y="-1" width="{}" height="{}"/>"#,
            fmt(center_x),
            fmt(center_y),
            escape_attr_display(dom_id),
            fmt(-lbl.width / 2.0),
            fmt(-lbl.height / 2.0),
            escape_attr_display(&background_style),
            fmt(lbl.width.max(0.0)),
            fmt(lbl.height.max(0.0)),
        );
        emitted_style = super::label::write_class_svg_edge_text_markdown_with_style(
            out,
            trimmed,
            ctx.text_paint.map(|paint| paint.style()),
        );
        out.push_str("</g></g></g>");
        if let Some(index) = relation_index.filter(|_| background_visible) {
            receipt.record_edge_label_background_paint(
                index,
                background.map(|(rule, _)| rule),
                &[(&background_style, "fill")],
            );
        }
    } else {
        out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
        let _ = write!(
            out,
            r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)">"#,
            escape_attr_display(dom_id)
        );
        emitted_style = super::label::write_class_svg_edge_text_with_style(
            out,
            trimmed,
            ctx.text_paint.map(|paint| paint.style()),
        );
        out.push_str("</g></g>");
    }
    let typography = if trimmed.is_empty() {
        crate::class::ClassTextTerminalFacts::default()
    } else {
        let facts = crate::class::class_svg_label_visible_style_facts(trimmed);
        crate::class::ClassTextTerminalFacts::from_visible_style_facts(&facts)
    };
    let typography = ctx
        .text_paint
        .map_or(typography, |paint| paint.observe(typography, emitted_style));
    (typography, background_visible)
}

fn render_class_edge_terminals(
    out: &mut impl SvgOutput,
    edge: &LayoutEdge,
    is_start: bool,
    content_bounds: &mut Option<Bounds>,
    ctx: &ClassEdgeGroupsRenderContext<'_>,
    typography_receipt: &mut Option<crate::class::ClassTextThemeReceipt>,
) -> crate::Result<()> {
    let Some(relation) = ctx.relations_by_id.get(edge.id.as_str()).copied() else {
        return Ok(());
    };
    let (text, labels) = if is_start {
        (
            relation.relation_title_1.as_deref().unwrap_or_default(),
            [&edge.start_label_left, &edge.start_label_right],
        )
    } else {
        (
            relation.relation_title_2.as_deref().unwrap_or_default(),
            [&edge.end_label_left, &edge.end_label_right],
        )
    };
    let decoded = decode_entities_minimal_cow(text);
    let text = decoded.trim();
    for (slot, label) in labels.into_iter().enumerate() {
        let mut typography = crate::class::ClassTextTerminalFacts::default();
        if let Some(label) = label
            .as_ref()
            .filter(|label| !text.is_empty() && label.width > 0.0 && label.height > 0.0)
        {
            // setTerminalWidth retains the legacy CSS width floor, but never clips the
            // measured text width or replaces its measured height with a fixed 12px box.
            let visible_width = if ctx.edge_use_html_labels {
                label.width.max(text.encode_utf16().count() as f64 * 9.0)
            } else {
                label.width
            };
            include_xywh(
                content_bounds,
                label.x - label.width / 2.0 + ctx.content_tx + ctx.bounds_dx,
                label.y - label.height / 2.0 + ctx.content_ty + ctx.bounds_dy,
                visible_width,
                label.height,
            );
            typography = render_class_edge_terminal_group(out, label, text, visible_width, ctx);
            out.checkpoint()?;
        }
        if let Some(receipt) = typography_receipt.as_mut() {
            receipt.record_cardinality(&edge.id, slot + if is_start { 0 } else { 2 }, typography);
        }
    }
    Ok(())
}

fn render_class_edge_terminal_group<O: SvgOutput>(
    out: &mut O,
    label: &LayoutLabel,
    text: &str,
    style_width: f64,
    ctx: &ClassEdgeGroupsRenderContext<'_>,
) -> crate::class::ClassTextTerminalFacts {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return crate::class::ClassTextTerminalFacts::default();
    }
    let x = label.x + ctx.content_tx;
    let y = label.y + ctx.content_ty;
    let emitted_style;
    if ctx.edge_use_html_labels {
        let _ = write!(
            out,
            r#"<g class="edgeTerminals" transform="translate({}, {})"><g class="inner" transform="translate({}, {})"><foreignObject width="{}" height="{}" style="width: {}px; height: {}px;"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5;">"#,
            fmt(x),
            fmt(y),
            fmt(-label.width / 2.0),
            fmt(-label.height / 2.0),
            fmt(label.width),
            fmt(label.height),
            fmt(style_width),
            fmt(label.height)
        );
        emitted_style = super::label::open_class_cardinality_label(
            out,
            ctx.text_paint.map(|paint| paint.style()),
        );
        render_class_terminal_label(out, trimmed, ctx.mermaid_config, ctx.math_renderer);
        out.push_str("</span></div></foreignObject></g></g>");
    } else {
        let bbox_y = ctx
            .text_measurer
            .measure_svg_create_text_bbox_y_offset_px(text, ctx.terminal_text_style);
        let _ = write!(
            out,
            r#"<g class="edgeTerminals" transform="translate({}, {})"><g class="inner" transform="translate(0, {})"><g><rect class="background" style="stroke: none"/>"#,
            fmt(x),
            fmt(y),
            fmt(-(bbox_y + label.height / 2.0))
        );
        emitted_style = ctx
            .text_paint
            .map(|paint| paint.style())
            .filter(|style| !style.is_empty());
        if let Some(style) = emitted_style {
            let _ = write!(out, r#"<g style="{}">"#, escape_attr_display(style));
        }
        write_class_svg_edge_text(out, text, false);
        if emitted_style.is_some() {
            out.push_str("</g>");
        }
        out.push_str("</g></g></g>");
    }
    let facts = if ctx.edge_use_html_labels && crate::math::contains_delimited_math(trimmed) {
        crate::class::ClassTextTerminalFacts::unverified_text(trimmed)
    } else {
        crate::class::ClassTextTerminalFacts::fixed_font_size_text(trimmed)
    };
    ctx.text_paint
        .map_or(facts, |paint| paint.observe(facts, emitted_style))
}

fn render_class_terminal_label(
    out: &mut impl SvgOutput,
    text: &str,
    mermaid_config: Option<&merman_core::MermaidConfig>,
    math_renderer: Option<&(dyn crate::math::MathRenderer + Send + Sync)>,
) {
    if let Some(math_html) = class_math_html_label(text, mermaid_config, math_renderer) {
        out.push_str(&math_html);
    } else {
        out.push_str("<p>");
        escape_xml_into(out, text);
        out.push_str("</p>");
    }
}

pub(super) fn class_edge_dom_id_into(
    out: &mut String,
    edge: &LayoutEdge,
    relation_index_by_id: &FxHashMap<&str, usize>,
) {
    out.clear();
    if edge.id.starts_with("edgeNote") {
        if let Some(note_idx) = edge
            .from
            .strip_prefix("note")
            .and_then(|rest| rest.parse::<usize>().ok())
        {
            let _ = write!(out, "edgeNote{note_idx}");
            return;
        }
        out.push_str(edge.id.as_str());
        return;
    }
    // Mermaid uses `getEdgeId` with prefix `id`.
    let idx = relation_index_by_id
        .get(edge.id.as_str())
        .copied()
        .unwrap_or(1);
    out.push_str("id_");
    out.push_str(edge.from.as_str());
    out.push('_');
    out.push_str(edge.to.as_str());
    out.push('_');
    let _ = write!(out, "{idx}");
}

pub(super) fn class_edge_pattern(line_type: i32) -> &'static str {
    // Mermaid class diagram `lineType` uses "dottedLine" for `..` which maps to the dashed pattern.
    if line_type == 1 {
        "edge-pattern-dashed"
    } else {
        "edge-pattern-solid"
    }
}

pub(super) fn class_note_edge_pattern() -> &'static str {
    "edge-pattern-dotted"
}

pub(super) fn class_edge_path_style(edge_id: &str, hand_drawn: bool) -> &'static str {
    if hand_drawn && edge_id.starts_with("edgeNote") {
        ";fill: none"
    } else if hand_drawn {
        ";"
    } else if edge_id.starts_with("edgeNote") {
        "fill: none;;;fill: none"
    } else {
        ";;;"
    }
}

pub(super) fn class_edge_render_order<'a>(
    edges: &'a [LayoutEdge],
    relation_index_by_id: &FxHashMap<&str, usize>,
) -> Vec<&'a LayoutEdge> {
    let mut ordered = edges.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| {
        let a_key = if a.id.starts_with("edgeNote") {
            (
                0_u8,
                a.id.trim_start_matches("edgeNote")
                    .parse::<usize>()
                    .unwrap_or(usize::MAX),
                a.id.as_str(),
            )
        } else {
            (
                1_u8,
                relation_index_by_id
                    .get(a.id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                a.id.as_str(),
            )
        };
        let b_key = if b.id.starts_with("edgeNote") {
            (
                0_u8,
                b.id.trim_start_matches("edgeNote")
                    .parse::<usize>()
                    .unwrap_or(usize::MAX),
                b.id.as_str(),
            )
        } else {
            (
                1_u8,
                relation_index_by_id
                    .get(b.id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                b.id.as_str(),
            )
        };
        a_key.cmp(&b_key)
    });
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label_at(x: f64, y: f64) -> LayoutLabel {
        LayoutLabel {
            x,
            y,
            width: 20.0,
            height: 16.0,
        }
    }

    #[test]
    fn class_edge_label_uses_pre_curve_points_when_cluster_clipping_updates_the_path() {
        let label_path_points = [
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 30.0, y: 0.0 },
            LayoutPoint { x: 100.0, y: 0.0 },
        ];

        let original_path_points = [
            LayoutPoint { x: -20.0, y: 0.0 },
            LayoutPoint { x: 100.0, y: 0.0 },
        ];
        // Preserve the provider's offset from the original midpoint when clipping moves it.
        let center = class_edge_label_center(
            Some(&original_path_points),
            &label_path_points,
            "M17.25,0L30,0L100,0",
            true,
            &label_at(12.0, 8.0),
            0.0,
            0.0,
        );

        assert_eq!((center.x, center.y), (22.0, 8.0));
    }

    #[test]
    fn class_edge_label_checks_the_actual_rendered_path_before_repositioning() {
        let label_path_points = [
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 10.0, y: 0.0 },
            LayoutPoint { x: 20.0, y: 0.0 },
        ];
        let label = label_at(4.0, 5.0);

        let unchanged = class_edge_label_center(
            Some(&label_path_points),
            &label_path_points,
            "M0,0L10,0L20,0",
            false,
            &label,
            100.0,
            200.0,
        );
        assert_eq!((unchanged.x, unchanged.y), (104.0, 205.0));

        let rough_path = class_edge_label_center(
            Some(&label_path_points),
            &label_path_points,
            "M1.25,2.5C3.5,4.5,5.5,6.5,7.5,8.5",
            false,
            &label,
            100.0,
            200.0,
        );
        assert_eq!((rough_path.x, rough_path.y), (104.0, 205.0));
    }
}
