//! Flowchart edge path renderer.

use super::super::defs::write_flowchart_marker_id_xml;
use super::super::*;

pub(in crate::svg::parity::flowchart) fn render_flowchart_edge_path(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
    scratch: &mut FlowchartEdgeDataPointsScratch,
    edge_cache: &mut FxHashMap<&str, FlowchartEdgePathCacheEntry>,
) {
    let trace_enabled = ctx.trace_edge_id.is_some_and(|id| id == edge.id.as_str());

    let cached_geom = edge_cache
        .get(edge.id.as_str())
        .filter(|c| (c.origin_x - origin_x).abs() <= 1e-9 && (c.origin_y - origin_y).abs() <= 1e-9)
        .map(|c| &c.geom);

    // Trace collection recomputes the pre-line-hop geometry for diagnostics, but the emitted SVG
    // must still consume the post-processed cache. Enabling diagnostics must not alter rendering.
    let owned_geom = if cached_geom.is_none() || trace_enabled {
        flowchart_compute_edge_path_geom(
            FlowchartEdgePathGeomRequest {
                ctx,
                edge,
                origin_x,
                origin_y,
                trace_enabled,
            },
            scratch,
        )
    } else {
        None
    };
    let geom = if let Some(g) = cached_geom {
        g
    } else {
        let Some(g) = owned_geom.as_ref() else {
            return;
        };
        g
    };
    let d = geom.d.as_str();
    let data_points_b64 = geom.data_points_b64.as_str();
    let data_look = flowchart_config_look(ctx.config);
    let hand_drawn = data_look == "handDrawn";
    let rough_d = if hand_drawn && !geom.line_hop_applied {
        super::node::roughjs::roughjs_hand_drawn_stroke_path_for_svg_path(
            d,
            0.3,
            &ctx.hand_drawn_seed,
        )
    } else {
        None
    };
    let d = rough_d.as_deref().unwrap_or(d);

    let mut marker_color: Option<&str> = None;
    for raw in ctx.default_edge_style.iter().chain(edge.style.iter()) {
        // Mirror Mermaid: handDrawn passes the full `stroke:...` style token to edgeMarker,
        // while classic/neo passes the captured color value from final pathStyle.
        let s = raw.trim_start();
        let Some(rest) = s.strip_prefix("stroke:") else {
            continue;
        };
        if !rest.trim().is_empty() {
            marker_color = Some(if hand_drawn { s } else { rest });
            break;
        }
    }

    // If no inline `stroke:` exists, Mermaid still colors markers based on class-derived stroke
    // styles (see `edges.js` `stylesFromClasses` + `edgeMarker.ts` `strokeColor` extraction).
    // We approximate this by compiling the edge styles using class defs and reusing the resulting
    // `stroke` value for the marker id suffix.
    let compiled_marker_color = if !hand_drawn && marker_color.is_none() && !edge.classes.is_empty()
    {
        flowchart_resolve_stroke_for_marker(
            ctx.class_defs,
            &edge.classes,
            &ctx.default_edge_style,
            &edge.style,
        )
    } else {
        None
    };
    if marker_color.is_none() {
        marker_color = compiled_marker_color.as_deref();
    }

    fn write_style_joined(out: &mut String, a: &[String], b: &[String]) {
        let mut first = true;
        for part in a.iter().chain(b.iter()) {
            if first {
                first = false;
            } else {
                out.push(';');
            }
            let _ = write!(out, "{}", escape_xml_display(part));
        }
    }

    let _ = write!(
        out,
        r#"<path d="{}" id="{}-{}" class=""#,
        d,
        escape_xml_display(ctx.diagram_id),
        escape_xml_display(&edge.id),
    );
    css::write_flowchart_edge_class_attr(out, edge);
    if hand_drawn {
        out.push_str(" transition");
    }
    out.push_str(r#"" style=""#);
    if data_look == "neo"
        && !flowchart_edge_is_animated(ctx, edge)
        && let Some(path_length) = flowchart_neo_edge_path_length(geom, edge)
    {
        write_flowchart_neo_edge_mask(out, path_length, edge, geom.line_hop_applied);
    }
    if hand_drawn {
        scratch.style_escaped.clear();
        write_style_joined(
            &mut scratch.style_escaped,
            &ctx.default_edge_style,
            &edge.style,
        );
        out.push_str(&scratch.style_escaped);
    } else if ctx.default_edge_style.is_empty() && edge.style.is_empty() {
        out.push(';');
    } else {
        scratch.style_escaped.clear();
        write_style_joined(
            &mut scratch.style_escaped,
            &ctx.default_edge_style,
            &edge.style,
        );
        out.push_str(&scratch.style_escaped);
        out.push_str(";;;");
        out.push_str(&scratch.style_escaped);
    }
    if hand_drawn {
        out.push_str(r##"" stroke="#000" stroke-width="1" fill="none"##);
    }
    let _ = write!(
        out,
        r#"" data-edge="true" data-et="edge" data-id="{}" data-points="{}" data-look="{}""#,
        escape_xml_display(&edge.id),
        data_points_b64,
        escape_xml_display(data_look),
    );
    if let Some(base) = flowchart_edge_marker_start_base(edge) {
        out.push_str(r#" marker-start="url(#"#);
        write_flowchart_marker_id_xml(out, ctx.diagram_id, ctx.diagram_type, base, marker_color);
        out.push_str(r#")""#);
    }
    if let Some(base) = flowchart_edge_marker_end_base(edge) {
        out.push_str(r#" marker-end="url(#"#);
        write_flowchart_marker_id_xml(out, ctx.diagram_id, ctx.diagram_type, base, marker_color);
        out.push_str(r#")""#);
    }
    out.push_str(" />");

    if let Some(emitted_d_for_label) = rough_d
        && let Some(cache_entry) = edge_cache.get_mut(edge.id.as_str())
        && (cache_entry.origin_x - origin_x).abs() <= 1e-9
        && (cache_entry.origin_y - origin_y).abs() <= 1e-9
    {
        cache_entry.geom.emitted_d_for_label = Some(emitted_d_for_label);
    }
}

fn flowchart_edge_is_animated(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
) -> bool {
    edge.animate == Some(true)
        || edge.animation.is_some()
        || edge
            .classes
            .iter()
            .filter_map(|class| ctx.class_defs.get(class))
            .flatten()
            .any(|declaration| declaration.contains("animation"))
}

fn flowchart_neo_edge_path_length(
    geom: &FlowchartEdgePathGeom,
    edge: &crate::flowchart::FlowEdge,
) -> Option<f64> {
    if geom.line_hop_applied && !matches!(edge.stroke.as_deref(), Some("dotted" | "dashed")) {
        geom.path_length
    } else {
        geom.original_path_length
    }
}

fn flowchart_neo_marker_mask_offset(arrow_type: Option<&str>) -> f64 {
    match arrow_type {
        Some("arrow_point") => 4.0,
        Some("arrow_cross" | "arrow_circle") => 12.5,
        _ => 0.0,
    }
}

fn write_flowchart_neo_edge_mask(
    out: &mut String,
    path_length: f64,
    edge: &crate::flowchart::FlowEdge,
    line_hop_applied: bool,
) {
    let (arrow_type_start, arrow_type_end) =
        super::super::edge_geom::arrow_types_for_edge(edge.edge_type.as_deref());
    let start_offset = flowchart_neo_marker_mask_offset(arrow_type_start);
    let end_offset = flowchart_neo_marker_mask_offset(arrow_type_end);
    let middle_length = if line_hop_applied {
        (path_length - start_offset - end_offset).max(0.0)
    } else {
        path_length - start_offset - end_offset
    };

    out.push_str("stroke-dasharray: 0 ");
    let _ = write!(out, "{} ", fmt(start_offset));
    if matches!(edge.stroke.as_deref(), Some("dotted" | "dashed")) {
        for _ in 0..(middle_length / 4.0).floor().max(0.0) as usize {
            out.push_str("2 2 ");
        }
    } else {
        let _ = write!(out, "{} ", fmt(middle_length));
    }
    let _ = write!(out, "{}; stroke-dashoffset: 0;", fmt(end_offset));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(edge_type: &str, stroke: &str) -> crate::flowchart::FlowEdge {
        crate::flowchart::FlowEdge {
            id: "edge".to_string(),
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            label_type: None,
            edge_type: Some(edge_type.to_string()),
            arrow: String::new(),
            is_user_defined_id: false,
            stroke: Some(stroke.to_string()),
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    #[test]
    fn neo_solid_mask_uses_final_line_hop_length_and_marker_offsets() {
        let mut style = String::new();
        write_flowchart_neo_edge_mask(&mut style, 40.0, &edge("arrow_point", "normal"), true);
        assert_eq!(style, "stroke-dasharray: 0 0 36 4; stroke-dashoffset: 0;");

        style.clear();
        write_flowchart_neo_edge_mask(
            &mut style,
            50.0,
            &edge("double_arrow_circle", "normal"),
            true,
        );
        assert_eq!(
            style,
            "stroke-dasharray: 0 12.5 25 12.5; stroke-dashoffset: 0;"
        );
    }

    #[test]
    fn neo_dotted_mask_preserves_upstream_two_pixel_pattern() {
        let mut style = String::new();
        write_flowchart_neo_edge_mask(&mut style, 16.0, &edge("arrow_open", "dotted"), false);
        assert_eq!(
            style,
            "stroke-dasharray: 0 0 2 2 2 2 2 2 2 2 0; stroke-dashoffset: 0;"
        );
    }
}
