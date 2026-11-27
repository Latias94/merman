//! Flowchart edge path renderer.

use super::super::defs::FlowchartMarkerEmissionPlan;
use super::super::edge_fill::FlowchartEdgeFillPlan;
use super::super::*;
use merman_render_flow::edge_svg::{FlowchartEdgeSvgEmission, FlowchartNeoEdgeMaskPlan};

#[cfg(test)]
use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};

#[allow(
    clippy::too_many_arguments,
    reason = "The streaming edge writer borrows independent prepared plans, shared scratch storage, and the geometry cache without allocating an aggregate context."
)]
pub(in crate::svg::parity::flowchart) fn render_flowchart_edge_path(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    marker_plan: &FlowchartMarkerEmissionPlan,
    fills: &FlowchartEdgeFillPlan,
    label_cutout: bool,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    origin_x: f64,
    origin_y: f64,
    scratch: &mut FlowchartEdgeDataPointsScratch,
    edge_cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
) -> crate::Result<()> {
    let key = edge_ref.key;
    let edge = edge_ref.edge;
    let edge_dom_id = ctx
        .document_ids()
        .edge(key)
        .ok_or_else(|| crate::Error::InvalidModel {
            message: format!(
                "missing prepared Flowchart DOM id for edge occurrence {}",
                key.semantic_index()
            ),
        })?;
    let trace_enabled = ctx.trace_edge_id.is_some_and(|id| id == edge.id.as_str());
    let data_look = ctx.compatibility.look;
    let hand_drawn = data_look.is_hand_drawn();
    let emitted_styles = ctx.edge_style_plan.edge_for(key)?;
    let animation = ctx.edge_style_plan.animation_for(key)?;
    let cached_geom = edge_cache
        .get(&key)
        .filter(|c| (c.origin_x - origin_x).abs() <= 1e-9 && (c.origin_y - origin_y).abs() <= 1e-9)
        .map(|c| &c.geom);

    // Trace collection recomputes the geometry before graph-wide postprocessing for diagnostics, but the emitted SVG
    // must still consume the post-processed cache. Enabling diagnostics must not alter rendering.
    let owned_geom = if cached_geom.is_none() || trace_enabled {
        if let Some(layout_edge) = ctx.layout_edges_by_key.get(&key) {
            ctx.work_meter.charge(layout_edge.points().len().max(2))?;
        }
        flowchart_compute_edge_path_geom(
            FlowchartEdgePathGeomRequest {
                ctx,
                key,
                edge,
                origin_x,
                origin_y,
                trace_enabled,
            },
            scratch,
        )?
    } else {
        None
    };
    let geom = if let Some(g) = cached_geom {
        g
    } else {
        let Some(g) = owned_geom.as_ref() else {
            return Ok(());
        };
        g
    };
    let d = geom.d.as_str();
    let data_points_b64 = scratch.data_points.encode(&geom.data_points);
    let neo_edge_mask = if data_look.is_neo()
        && !animation.is_active()
        && let Some(path_length) = flowchart_neo_edge_path_length(geom, edge)
    {
        Some(FlowchartNeoEdgeMaskPlan::prepare(
            path_length,
            edge,
            geom.line_hop_applied,
            ctx.work_meter,
        )?)
    } else {
        None
    };
    let d = geom.emitted_d_for_label.as_deref().unwrap_or(d);

    scratch.edge_class_attr.clear();
    css::write_flowchart_edge_class_attr(
        &mut scratch.edge_class_attr,
        edge,
        animation,
        ctx.layout.route_contract.is_canonical_axis(),
    );
    if hand_drawn {
        scratch.edge_class_attr.push_str(" transition");
    }
    scratch.edge_marker_attrs.clear();
    marker_plan.push_edge_marker_attributes_for(
        &mut scratch.edge_marker_attrs,
        ctx.document_ids().marker_scope(),
        ctx.diagram_type,
        key,
        ctx.work_meter,
    )?;
    if label_cutout {
        super::super::edge_label_cutout::write_attribute(
            &mut scratch.edge_marker_attrs,
            edge_dom_id,
        );
    }
    // Mermaid's legacy edge writer appends the same inline list twice. Keep its byte contract
    // only for parity renderers; the native painter needs the declarations once in cascade order.
    let repeat_inline = !ctx.layout.route_contract.is_canonical_axis();
    let source_style_svg_bytes = ctx
        .edge_style_plan
        .edge_source_style_svg_bytes_for(key, repeat_inline)?;
    let path = FlowchartEdgeSvgEmission {
        edge_dom_id,
        edge,
        d,
        data_points_b64,
        data_look,
        hand_drawn,
        class_declarations: emitted_styles.emitted_edge_class_declarations(),
        class_attr: &scratch.edge_class_attr,
        marker_attrs: &scratch.edge_marker_attrs,
        default_edge_style: ctx.default_edge_style,
        repeat_inline,
        adaptive_ink: fills.ink(key),
        neo_edge_mask,
        line_hop_applied: geom.line_hop_applied,
    };
    ctx.checkpoint_emit()?;
    let rewritten_style = path.append_to(out, source_style_svg_bytes, ctx.work_meter)?;
    for layer in fills.layers(key) {
        // Use the very same shaft, dash pattern, setback, and marker viewport at full width.
        // The group clip applies to the markers too, including the parts across a surface seam.
        ctx.work_meter.charge_emit_work(d.len().div_ceil(16) + 1)?;
        out.push_str(r#"<g class="merman-edge-surface-ink" aria-hidden="true" pointer-events="none" clip-path="url(#"#);
        let _ = FlowchartEdgeFillPlan::write_clip_id(out, ctx, layer.clip);
        out.push_str(r#")">"#);
        path.write_surface_path(out, &layer.color, rewritten_style.as_deref())?;
        if label_cutout {
            super::super::edge_label_cutout::write_attribute(out, edge_dom_id);
        }
        marker_plan.push_surface_marker_attributes_for(
            out,
            ctx.document_ids().marker_scope(),
            ctx.diagram_type,
            (key, layer.clip),
            ctx.work_meter,
        )?;
        out.push_str("/></g>");
        out.checkpoint()?;
    }
    super::super::step::write_edge_marks(out, ctx, key, edge, &geom.d)?;
    Ok(())
}

fn flowchart_neo_edge_path_length(
    geom: &FlowchartEdgePathGeom,
    edge: &crate::flowchart::FlowEdge,
) -> Option<f64> {
    if geom.line_hop_applied && flowchart_neo_mask_uses_dash_pairs(edge) {
        geom.pre_hop_dash_mask_length
    } else {
        geom.centerline_length()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{
        RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
    };
    use crate::svg::parity::flowchart::document_ids::{
        FlowchartDocumentIdRequest, FlowchartDocumentIds,
    };
    use crate::svg::parity::flowchart::render_input::FlowchartRenderEdgeRef;
    use crate::svg::parity::{BoundedSvgOutput, SvgOutput};
    use indexmap::IndexMap;

    fn edge(edge_type: &str, stroke: &str) -> crate::flowchart::FlowEdge {
        crate::flowchart::FlowEdge {
            id: "edge".to_string(),
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            label_type: None,
            edge_type: Some(edge_type.to_string()),
            arrow: String::new(),
            start_marker: FlowEdgeMarker::None,
            end_marker: if edge_type.starts_with("arrow_open") {
                FlowEdgeMarker::None
            } else {
                FlowEdgeMarker::Point
            },
            is_user_defined_id: false,
            stroke: Some(stroke.to_string()),
            stroke_kind: match stroke {
                "dotted" => FlowEdgeStroke::Dotted,
                "thick" => FlowEdgeStroke::Thick,
                _ => FlowEdgeStroke::Normal,
            },
            visibility: if stroke == "invisible" {
                FlowEdgeVisibility::Invisible
            } else {
                FlowEdgeVisibility::Visible
            },
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            step: None,
            length: 1,
        }
    }

    fn meter_with_limits(
        svg_bytes: usize,
        work_units: usize,
    ) -> crate::resources::OperationWorkMeter {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, svg_bytes)
            .unwrap()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, work_units)
            .unwrap();
        crate::resources::OperationWorkMeter::new(policy)
    }

    fn neo_mask(
        path_length: f64,
        edge: &crate::flowchart::FlowEdge,
        line_hop_applied: bool,
    ) -> FlowchartNeoEdgeMaskPlan {
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        FlowchartNeoEdgeMaskPlan::prepare(path_length, edge, line_hop_applied, &meter)
            .expect("prepare Neo edge mask")
    }

    #[test]
    fn complete_edge_is_admitted_before_any_path_output() {
        let class_defs =
            IndexMap::from([("accent".to_string(), vec!["stroke:#2563eb".to_string()])]);
        let default_edge_style = vec!["opacity:0.5".to_string()];
        let mut edge = edge("arrow_open", "dotted");
        edge.id = "\u{8fb9}#quot;&<]\u{1}]>".to_string();
        edge.classes.push("accent".to_string());
        edge.style.push("stroke:#ef4444".to_string());
        let emitted_styles =
            flowchart_compile_styles(&class_defs, &edge.classes, &default_edge_style, &edge.style)
                .into_edge_artifact(false);
        let source_style_svg_bytes = emitted_styles
            .projected_edge_source_style_bytes(&default_edge_style, &edge.style, false)
            .expect("finite source-style projection")
            .parity;
        let neo_edge_mask = neo_mask(16.0, &edge, false);
        let key = crate::flowchart::FlowchartEdgeKey::self_loop_segment(12, 2);
        let document_ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
            diagram_id: merman_render_core::svg_css::SvgId::new("diagram").unwrap(),
            edge_order: &[FlowchartRenderEdgeRef { key, edge: &edge }],
            node_dom_index: &FxHashMap::default(),
            subgraph_index_by_id: &FxHashMap::default(),
            layout_nodes: &[],
            swimlane_nodes: None,
            swimlane_lanes: None,
        });
        let emission = FlowchartEdgeSvgEmission {
            edge_dom_id: document_ids.edge(key).unwrap(),
            edge: &edge,
            d: "M0,0L16,0",
            data_points_b64: "W3sieCI6MH1d",
            data_look: crate::config::DiagramLook::from_raw(Some("neo")),
            hand_drawn: false,
            class_declarations: emitted_styles.emitted_edge_class_declarations(),
            class_attr: "edge-thickness-normal edge-pattern-dotted edge-thickness-normal edge-pattern-solid flowchart-link",
            marker_attrs: r#" marker-start="url(#diagram-flowchart-v2-circleStart)" marker-end="url(#diagram-flowchart-v2-pointEnd)""#,
            default_edge_style: &default_edge_style,
            repeat_inline: true,
            adaptive_ink: None,
            neo_edge_mask: Some(neo_edge_mask),
            line_hop_applied: false,
        };
        let expected_edge = r##"<path d="M0,0L16,0" id="diagram-merman-flowchart-document-edge-12-segment-2" class="edge-thickness-normal edge-pattern-dotted edge-thickness-normal edge-pattern-solid flowchart-link" style="stroke-dasharray: 0 0 2 2 2 2 2 2 2 2 0; stroke-dashoffset: 0;stroke:#2563eb;opacity:0.5;stroke:#ef4444;;;opacity:0.5;stroke:#ef4444" data-edge="true" data-et="edge" data-id="边&quot;&amp;&lt;]]&gt;" data-points="W3sieCI6MH1d" data-look="neo" marker-start="url(#diagram-flowchart-v2-circleStart)" marker-end="url(#diagram-flowchart-v2-pointEnd)" />"##;
        let initial = "prefix";
        let probe_meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let exact_bytes = initial.len() + expected_edge.len();
        let projected_style_bytes = source_style_svg_bytes + neo_edge_mask.serialized_bytes();
        let exact_meter = meter_with_limits(exact_bytes, 4);
        let mut exact = BoundedSvgOutput::new(&exact_meter);
        exact.push_str(initial);
        emission
            .append_to(&mut exact, source_style_svg_bytes, &exact_meter)
            .expect("exact whole-edge budgets must pass");
        assert_eq!(exact.finish().unwrap(), format!("{initial}{expected_edge}"));
        assert_eq!(exact_meter.used(), 4);
        assert_eq!(exact_meter.projected_svg_bytes(), projected_style_bytes);

        for maximum in [exact_bytes - 1, initial.len() + expected_edge.len() / 2] {
            let short_svg_meter = meter_with_limits(maximum, 5);
            let mut short_svg = BoundedSvgOutput::new(&short_svg_meter);
            short_svg.push_str(initial);
            let error = emission
                .append_to(&mut short_svg, source_style_svg_bytes, &short_svg_meter)
                .expect_err("short SVG budget must reject before path emission");
            let crate::Error::ResourceLimitExceeded(error) = error else {
                panic!("expected structured SVG resource rejection");
            };
            assert_eq!(error.limit, "max_svg_bytes");
            assert_eq!(error.phase, ResourceLimitPhase::SvgOutput);
            assert_eq!(error.cause, ResourceLimitCause::Ceiling);
            assert_eq!((error.actual, error.max), (exact_bytes, maximum));
            assert_eq!(short_svg.finish().unwrap(), initial);
            assert_eq!(short_svg_meter.used(), 0);
            assert_eq!(short_svg_meter.projected_svg_bytes(), 0);
        }

        let short_work_meter = meter_with_limits(exact_bytes - 1, 3);
        let mut short_work = BoundedSvgOutput::new(&short_work_meter);
        short_work.push_str(initial);
        let error = emission
            .append_to(&mut short_work, source_style_svg_bytes, &short_work_meter)
            .expect_err("short work budget must reject before path emission");
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured work resource rejection");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!((error.actual, error.max), (4, 3));
        assert_eq!(short_work.as_str(), initial);
        assert_eq!(short_work_meter.used(), 0);
        assert_eq!(short_work_meter.projected_svg_bytes(), 0);

        let hopped = FlowchartEdgeSvgEmission {
            d: "M0,0L20,0",
            line_hop_applied: true,
            ..emission
        };
        let mut hopped_svg = initial.to_string();
        hopped
            .append_to(&mut hopped_svg, source_style_svg_bytes, &probe_meter)
            .expect("rewritten line-hop mask must preserve source styles");
        assert!(hopped_svg.contains("stroke-dasharray: 0 0 18 2;"));
        assert!(hopped_svg.contains("stroke:#ef4444"));
        let short_hopped_meter = meter_with_limits(hopped_svg.len() - 1, usize::MAX);
        let mut short_hopped = BoundedSvgOutput::new(&short_hopped_meter);
        short_hopped.push_str(initial);
        let error = hopped
            .append_to(
                &mut short_hopped,
                source_style_svg_bytes,
                &short_hopped_meter,
            )
            .expect_err("rewritten style must be admitted before path emission");
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured SVG resource rejection");
        };
        assert_eq!(error.actual, hopped_svg.len());
        assert_eq!(short_hopped.as_str(), initial);
        assert_eq!(
            short_hopped_meter.used(),
            projected_style_bytes + hopped.d.len()
        );
        assert_eq!(short_hopped_meter.projected_svg_bytes(), 0);

        let hand_drawn = FlowchartEdgeSvgEmission {
            hand_drawn: true,
            data_look: crate::config::DiagramLook::from_raw(Some("handDrawn")),
            neo_edge_mask: None,
            ..emission
        };
        let hand_drawn_style_bytes = emitted_styles
            .projected_edge_source_style_bytes(&default_edge_style, &edge.style, true)
            .unwrap()
            .parity;
        let mut hand_drawn_svg = String::new();
        hand_drawn
            .append_to(&mut hand_drawn_svg, hand_drawn_style_bytes, &probe_meter)
            .unwrap();
        assert!(hand_drawn_svg.contains(
            r##"style="opacity:0.5;stroke:#ef4444" stroke="#000" stroke-width="1" fill="none""##
        ));
        let overflow_meter = meter_with_limits(exact_bytes, 4);
        let mut overflow = BoundedSvgOutput::new(&overflow_meter);
        overflow.push_str(initial);
        let error = hand_drawn
            .append_to(&mut overflow, usize::MAX, &overflow_meter)
            .expect_err("complete edge arithmetic must report overflow");
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured SVG arithmetic rejection");
        };
        assert_eq!(error.limit, "max_svg_bytes");
        assert_eq!(error.phase, ResourceLimitPhase::SvgOutput);
        assert_eq!(error.cause, ResourceLimitCause::ArithmeticOverflow);
        assert_eq!((error.actual, error.max), (usize::MAX, exact_bytes));
        assert_eq!(overflow.as_str(), initial);
        assert_eq!(overflow_meter.used(), 0);
        assert_eq!(overflow_meter.projected_svg_bytes(), 0);

        let cancelled_meter = meter_with_limits(exact_bytes - 1, 3);
        let mut cancelled = BoundedSvgOutput::new(&cancelled_meter);
        cancelled.push_str(initial);
        cancelled_meter.control().cancel();
        let error = emission
            .append_to(&mut cancelled, source_style_svg_bytes, &cancelled_meter)
            .expect_err("cancellation at work preflight must win over both ceilings");
        assert!(matches!(error, crate::Error::Cancelled(details)
            if details.phase == merman_model::OperationPhase::Layout));
        assert_eq!(cancelled.as_str(), initial);
        assert_eq!(cancelled_meter.used(), 0);
        assert_eq!(cancelled_meter.projected_svg_bytes(), 0);

        let long_d = format!("M0,0{}", "L1,1".repeat(800));
        let long_edge = FlowchartEdgeSvgEmission {
            d: &long_d,
            ..emission
        };
        let interrupted_meter = meter_with_limits(usize::MAX, 4);
        let mut interrupted = BoundedSvgOutput::new(&interrupted_meter);
        interrupted.push_str(initial);
        // Allow the preflight, whole-path check, and both charges. The long `d` is then
        // the first fragment to reach the sink's cancellation observation interval.
        interrupted_meter.control().cancel_after_checkpoints(4);
        let error = long_edge
            .append_to(&mut interrupted, source_style_svg_bytes, &interrupted_meter)
            .expect_err("bounded emission must observe cancellation during the path");
        let crate::Error::Cancelled(cancellation) = error else {
            panic!("expected structured cancellation from the bounded writer");
        };
        assert_eq!(cancellation.phase, merman_model::OperationPhase::Emit);
        assert_eq!(interrupted.as_str(), format!(r#"{initial}<path d=""#));
        assert_eq!(interrupted_meter.used(), 4);
        assert_eq!(
            interrupted_meter.projected_svg_bytes(),
            projected_style_bytes
        );
        assert!(
            matches!(interrupted.finish(), Err(crate::Error::Cancelled(error))
            if error == cancellation)
        );
    }
}
