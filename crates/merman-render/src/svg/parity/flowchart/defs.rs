//! Flowchart SVG defs and marker emission.

use std::fmt::Write as _;

use super::super::SvgDiagramId;
use super::super::markers::{push_base_edge_markers, push_edge_marker};
use super::super::util::escape_xml_display;
use super::{
    FlowchartRenderCtx, flowchart_config_look, flowchart_edge_is_animated,
    flowchart_edge_marker_end_base, flowchart_edge_marker_start_base,
    flowchart_resolve_stroke_for_marker,
};

struct ColoredMarker {
    base: &'static str,
    margin: bool,
    color_id: String,
    color: String,
}

pub(in crate::svg::parity::flowchart) struct FlowchartDefs<'a> {
    diagram_id: SvgDiagramId<'a>,
    diagram_type: &'a str,
    extra_markers: Vec<ColoredMarker>,
    security_level_loose: bool,
}

pub(in crate::svg::parity::flowchart) fn prepare_flowchart_defs<'a>(
    diagram_id: SvgDiagramId<'a>,
    diagram_type: &'a str,
    ctx: &FlowchartRenderCtx<'_>,
) -> FlowchartDefs<'a> {
    FlowchartDefs {
        diagram_id,
        diagram_type,
        extra_markers: collect_edge_markers(ctx),
        security_level_loose: ctx.security_level_loose,
    }
}

impl FlowchartDefs<'_> {
    pub(in crate::svg::parity::flowchart) fn push_base_markers(&self, out: &mut String) {
        push_base_edge_markers(out, self.diagram_id, self.diagram_type);
    }

    pub(in crate::svg::parity::flowchart) fn push_extra_markers(&self, out: &mut String) {
        for marker in &self.extra_markers {
            let color = if self.security_level_loose {
                marker.color.as_str()
            } else {
                // DOMPurify trims ordinary SVG attribute values in strict/sandbox output.
                marker.color.trim()
            };
            push_edge_marker(
                out,
                self.diagram_id,
                self.diagram_type,
                marker.base,
                marker.margin,
                Some((&marker.color_id, color)),
            );
        }
    }
}

fn marker_color_id(color: &str) -> String {
    // Mermaid's DOM marker id coloring logic (Mermaid 12) uses:
    // `strokeColor.replace(/[^\dA-Za-z]/g, '_')`
    //
    // Important: this does not trim whitespace. As a result, values like `" orange"` (leading
    // space captured from `style="...stroke: orange;..."`) produce a leading `_` in the color id,
    // which in turn yields a `__orange` suffix in the final marker id.
    let raw = color.trim_end_matches(';');
    if raw.trim().is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

#[inline]
pub(in crate::svg::parity::flowchart) fn write_flowchart_marker_id_xml(
    out: &mut String,
    diagram_id: SvgDiagramId<'_>,
    diagram_type: &str,
    base: &str,
    margin: bool,
    color: Option<&str>,
) {
    let _ = write!(out, "{diagram_id}");
    out.push('_');
    let _ = write!(out, "{}", escape_xml_display(diagram_type));
    out.push('-');
    out.push_str(base);
    if margin {
        out.push_str("-margin");
    }

    let Some(color) = color else {
        return;
    };
    let raw = color.trim_end_matches(';');
    if raw.trim().is_empty() {
        return;
    }
    out.push('_');
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
}

fn collect_edge_markers(ctx: &FlowchartRenderCtx<'_>) -> Vec<ColoredMarker> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let look = flowchart_config_look(ctx.config);
    let hand_drawn = look == "handDrawn";

    for edge_id in &ctx.edge_order {
        let Some(edge) = ctx.edges_by_id.get(edge_id) else {
            continue;
        };
        let mut color = None;
        for raw in ctx.default_edge_style.iter().chain(edge.style.iter()) {
            // Hand-drawn edges pass the whole stroke token to edgeMarker.ts.
            let token = raw.trim_start();
            let Some(value) = token.strip_prefix("stroke:") else {
                continue;
            };
            if !value.trim().is_empty() {
                color = Some(if hand_drawn { token } else { value }.to_owned());
                break;
            }
        }
        if !hand_drawn && color.is_none() && !edge.classes.is_empty() {
            color = flowchart_resolve_stroke_for_marker(
                ctx.class_defs,
                &edge.classes,
                &ctx.default_edge_style,
                &edge.style,
            );
        }
        let Some(color) = color else { continue };
        let color_id = marker_color_id(&color);
        if color_id.is_empty() {
            continue;
        }
        let margin = look == "neo" && !flowchart_edge_is_animated(ctx, edge);
        for base in [
            flowchart_edge_marker_start_base(edge),
            flowchart_edge_marker_end_base(edge),
        ]
        .into_iter()
        .flatten()
        {
            if seen.insert((base, margin, color_id.clone())) {
                out.push(ColoredMarker {
                    base,
                    margin,
                    color_id: color_id.clone(),
                    color: color.trim_end_matches(';').to_owned(),
                });
            }
        }
    }
    out
}
