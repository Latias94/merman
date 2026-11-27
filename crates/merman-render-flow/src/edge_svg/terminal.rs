//! Edge paint on geometry supplied by the Flow layout adapter.

use merman_model::{OperationPhase, families::flowchart::FlowEdge};
use merman_render_core::{
    Error, LayoutPoint, Result, resources::OperationWorkMeter,
    svg_edge_parity::svg_path_length_from_d, svg_output::SvgOutput,
};

use super::{FlowchartEdgeDataPoints, FlowchartEdgeSvgEmission, FlowchartNeoEdgeMaskPlan};
use crate::{
    document::{FlowchartDocumentId, FlowchartDocumentIds},
    markers::{FlowchartMarkerEmissionPlan, FlowchartMarkerSurface},
    projection::FlowchartRenderEdgeRef,
    style::{
        FlowchartEdgeAnimationResolution, FlowchartEdgeStylePlan, compiled::FlowchartCompiledStyles,
    },
    theme::FlowchartCompatibilityBinding,
};

/// Logical and painted paths from the same prepared layout occurrence.
pub struct FlowchartEdgeSvgGeometry<'a> {
    pub d: &'a str,
    pub emitted_d_for_label: Option<&'a str>,
    pub data_points: &'a [LayoutPoint],
    pub neo_length: Option<f64>,
    pub pre_hop_dash_mask_length: Option<f64>,
    pub line_hop_applied: bool,
}

impl FlowchartEdgeSvgGeometry<'_> {
    fn neo_edge_path_length(&self, edge: &FlowEdge) -> Option<f64> {
        if self.line_hop_applied && super::flowchart_neo_mask_uses_dash_pairs(edge) {
            self.pre_hop_dash_mask_length
        } else {
            self.neo_length.or_else(|| svg_path_length_from_d(self.d))
        }
    }
}

/// Reusable serialization buffers for one graph edge pass.
#[derive(Default)]
pub struct FlowchartEdgeSvgScratch {
    data_points: FlowchartEdgeDataPoints,
    class_attr: String,
    marker_attrs: String,
}

/// Document-local paint inputs from the same render and work meter.
pub struct FlowchartEdgeSvgContext<'a> {
    pub document_ids: &'a FlowchartDocumentIds,
    pub marker_plan: &'a FlowchartMarkerEmissionPlan<'a>,
    pub style_plan: &'a FlowchartEdgeStylePlan,
    pub compatibility: &'a FlowchartCompatibilityBinding,
    pub steps: &'a super::FlowchartSteps,
    pub default_edge_style: &'a [String],
    pub adaptive_ink: Option<&'static str>,
    pub surface_layers: &'a [FlowchartMarkerSurface],
    pub label_cutout: bool,
    pub canonical: bool,
    pub work_meter: &'a OperationWorkMeter,
}

impl<'a> FlowchartEdgeSvgContext<'a> {
    /// Resolve the required paint rows before the caller selects or recomputes geometry.
    pub fn bind(self, edge_ref: FlowchartRenderEdgeRef<'a>) -> Result<FlowchartEdgeTerminal<'a>> {
        let key = edge_ref.key;
        let edge_dom_id = self
            .document_ids
            .edge(key)
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Flowchart DOM id for edge occurrence {}",
                    key.semantic_index()
                ),
            })?;
        let emitted_styles = self.style_plan.edge_for(key)?;
        let animation = self.style_plan.animation_for(key)?;
        Ok(FlowchartEdgeTerminal {
            context: self,
            edge_ref,
            edge_dom_id,
            emitted_styles,
            animation,
        })
    }
}

/// The bound paint for an occurrence, ready to consume the adapter's borrowed geometry.
pub struct FlowchartEdgeTerminal<'a> {
    context: FlowchartEdgeSvgContext<'a>,
    edge_ref: FlowchartRenderEdgeRef<'a>,
    edge_dom_id: FlowchartDocumentId<'a>,
    emitted_styles: &'a FlowchartCompiledStyles,
    animation: FlowchartEdgeAnimationResolution,
}

impl FlowchartEdgeTerminal<'_> {
    pub fn append_to(
        self,
        out: &mut impl SvgOutput,
        geometry: FlowchartEdgeSvgGeometry<'_>,
        scratch: &mut FlowchartEdgeSvgScratch,
    ) -> Result<()> {
        self.append_with_grid_hop(out, geometry, None, scratch)
    }

    pub(crate) fn append_with_grid_hop(
        self,
        out: &mut impl SvgOutput,
        geometry: FlowchartEdgeSvgGeometry<'_>,
        grid_hopped_d: Option<&str>,
        scratch: &mut FlowchartEdgeSvgScratch,
    ) -> Result<()> {
        let ctx = &self.context;
        let FlowchartRenderEdgeRef { key, edge } = self.edge_ref;
        let data_look = ctx.compatibility.look;
        let hand_drawn = data_look.is_hand_drawn();
        let data_points_b64 = scratch.data_points.encode(geometry.data_points);
        let neo_edge_mask = if data_look.is_neo()
            && !self.animation.is_active()
            && let Some(path_length) = geometry.neo_edge_path_length(edge)
        {
            Some(FlowchartNeoEdgeMaskPlan::prepare(
                path_length,
                edge,
                geometry.line_hop_applied,
                ctx.work_meter,
            )?)
        } else {
            None
        };
        let d = grid_hopped_d
            .or(geometry.emitted_d_for_label)
            .unwrap_or(geometry.d);

        scratch.class_attr.clear();
        write_flowchart_edge_class_attr(
            &mut scratch.class_attr,
            edge,
            self.animation,
            ctx.canonical,
        );
        if hand_drawn {
            scratch.class_attr.push_str(" transition");
        }
        scratch.marker_attrs.clear();
        ctx.marker_plan
            .push_edge_marker_attributes_for(&mut scratch.marker_attrs, key)?;
        if ctx.label_cutout {
            crate::svg::edge_label_cutout::write_attribute(
                &mut scratch.marker_attrs,
                self.edge_dom_id,
            );
        }
        // Mermaid parity preserves the legacy duplicate inline list. Canonical SVG emits it once.
        let repeat_inline = !ctx.canonical;
        let source_style_svg_bytes = ctx
            .style_plan
            .edge_source_style_svg_bytes_for(key, repeat_inline)?;
        let path = FlowchartEdgeSvgEmission {
            edge_dom_id: self.edge_dom_id,
            edge,
            d,
            data_points_b64,
            data_look,
            hand_drawn,
            class_declarations: self.emitted_styles.emitted_edge_class_declarations(),
            class_attr: &scratch.class_attr,
            marker_attrs: &scratch.marker_attrs,
            default_edge_style: ctx.default_edge_style,
            repeat_inline,
            adaptive_ink: ctx.adaptive_ink,
            neo_edge_mask,
            line_hop_phase: if grid_hopped_d.is_some() {
                Some(OperationPhase::Emit)
            } else {
                geometry.line_hop_applied.then_some(OperationPhase::Layout)
            },
        };
        ctx.work_meter.checkpoint(OperationPhase::Emit)?;
        let rewritten_style = path.append_to(out, source_style_svg_bytes, ctx.work_meter)?;
        for surface in ctx.surface_layers {
            ctx.work_meter.charge_emit_work(d.len().div_ceil(16) + 1)?;
            out.push_str(r#"<g class="merman-edge-surface-ink" aria-hidden="true" pointer-events="none" clip-path="url(#"#);
            let _ = crate::svg::edge_fill::FlowchartEdgeFillPlan::write_clip_id(
                out,
                ctx.document_ids.scope().as_str(),
                surface.clip,
            );
            out.push_str(r#")">"#);
            path.write_surface_path(out, &surface.color, rewritten_style.as_deref())?;
            if ctx.label_cutout {
                crate::svg::edge_label_cutout::write_attribute(out, self.edge_dom_id);
            }
            ctx.marker_plan
                .push_surface_marker_attributes_for(out, (key, surface.clip))?;
            out.push_str("/></g>");
            out.checkpoint()?;
        }
        super::step::write_edge_marks(out, ctx, self.edge_ref, geometry.d)
    }
}

#[inline]
fn write_flowchart_edge_class_attr(
    out: &mut impl SvgOutput,
    edge: &FlowEdge,
    animation: FlowchartEdgeAnimationResolution,
    canonical: bool,
) {
    // Mermaid includes a 2-part class tuple (thickness/pattern) for flowchart edge paths. The
    // second tuple is `edge-thickness-normal edge-pattern-solid` in Mermaid@11.12.2 baselines,
    // even for dotted/thick strokes.
    let (thickness_1, pattern_1) = match edge.stroke.as_deref() {
        Some("thick") => ("edge-thickness-thick", "edge-pattern-solid"),
        Some("invisible") => ("edge-thickness-invisible", "edge-pattern-solid"),
        Some("dotted") => ("edge-thickness-normal", "edge-pattern-dotted"),
        _ => ("edge-thickness-normal", "edge-pattern-solid"),
    };

    if thickness_1 == "edge-thickness-invisible" {
        // Mermaid@11.12.2 does *not* include the second tuple nor `flowchart-link` for invisible
        // edges.
        out.push_str(thickness_1);
        out.push(' ');
        out.push_str(pattern_1);
        return;
    }

    out.push_str(thickness_1);
    out.push(' ');
    out.push_str(pattern_1);
    if !canonical {
        out.push_str(" edge-thickness-normal edge-pattern-solid");
    }
    out.push_str(" flowchart-link");

    if let Some(cls) = animation.class() {
        out.push(' ');
        out.push_str(cls);
    }
}
