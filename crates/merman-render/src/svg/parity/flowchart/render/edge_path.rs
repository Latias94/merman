//! Flowchart edge path renderer.

use super::super::defs::FlowchartMarkerEmissionPlan;
use super::super::document_ids::FlowchartDocumentId;
use super::super::*;
use crate::svg::parity::util::escaped_xml_len;
use std::fmt;

#[cfg(test)]
use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};

const PATH_START: &str = r#"<path d=""#;
const ID_ATTRIBUTE: &str = r#"" id=""#;
const CLASS_ATTRIBUTE: &str = r#"" class=""#;
const STYLE_ATTRIBUTE: &str = r#"" style=""#;
const HAND_DRAWN_ATTRIBUTES: &str = r##"" stroke="#000" stroke-width="1" fill="none"##;
const DATA_ID_ATTRIBUTE: &str = r#"" data-edge="true" data-et="edge" data-id=""#;
const DATA_POINTS_ATTRIBUTE: &str = r#"" data-points=""#;
const DATA_LOOK_ATTRIBUTE: &str = r#"" data-look=""#;
const ATTRIBUTE_END: &str = "\"";
const PATH_END: &str = " />";
const FIXED_PATH_BYTES: usize = PATH_START.len()
    + ID_ATTRIBUTE.len()
    + CLASS_ATTRIBUTE.len()
    + STYLE_ATTRIBUTE.len()
    + DATA_ID_ATTRIBUTE.len()
    + DATA_POINTS_ATTRIBUTE.len()
    + DATA_LOOK_ATTRIBUTE.len()
    + ATTRIBUTE_END.len()
    + PATH_END.len();

const NEO_EDGE_MASK_PREFIX: &str = "stroke-dasharray: 0 ";
const NEO_EDGE_MASK_DASH_PAIR: &str = "2 2 ";
const NEO_EDGE_MASK_SUFFIX: &str = "; stroke-dashoffset: 0;";

#[allow(
    clippy::too_many_arguments,
    reason = "The streaming edge writer borrows independent prepared plans, shared scratch storage, and the geometry cache without allocating an aggregate context."
)]
pub(in crate::svg::parity::flowchart) fn render_flowchart_edge_path(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    marker_plan: &FlowchartMarkerEmissionPlan,
    adaptive_ink: Option<&'static str>,
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
    scratch.json.clear();
    json_stringify_points_into(&mut scratch.json, &geom.data_points, &mut scratch.ryu);
    scratch.data_points_base64.clear();
    base64::engine::general_purpose::STANDARD
        .encode_string(scratch.json.as_bytes(), &mut scratch.data_points_base64);
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
    css::write_flowchart_edge_class_attr(&mut scratch.edge_class_attr, edge, animation);
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
    let source_style_svg_bytes = ctx.edge_style_plan.edge_source_style_svg_bytes_for(key)?;
    let path = FlowchartEdgeSvgEmission {
        edge_dom_id,
        edge,
        d,
        data_points_b64: &scratch.data_points_base64,
        data_look,
        hand_drawn,
        emitted_styles,
        class_attr: &scratch.edge_class_attr,
        marker_attrs: &scratch.edge_marker_attrs,
        default_edge_style: ctx.default_edge_style,
        adaptive_ink,
        neo_edge_mask,
        line_hop_applied: geom.line_hop_applied,
    };
    ctx.checkpoint_emit()?;
    path.append_to(out, source_style_svg_bytes, ctx.work_meter)?;
    super::super::step::write_edge_marks(out, ctx, key, edge, &geom.d)?;
    Ok(())
}

struct FlowchartEdgeSvgEmission<'a> {
    edge_dom_id: FlowchartDocumentId<'a>,
    edge: &'a crate::flowchart::FlowEdge,
    d: &'a str,
    data_points_b64: &'a str,
    data_look: crate::config::DiagramLook<'a>,
    hand_drawn: bool,
    emitted_styles: &'a FlowchartCompiledStyles,
    class_attr: &'a str,
    marker_attrs: &'a str,
    default_edge_style: &'a [String],
    adaptive_ink: Option<&'static str>,
    neo_edge_mask: Option<FlowchartNeoEdgeMaskPlan>,
    line_hop_applied: bool,
}

impl FlowchartEdgeSvgEmission<'_> {
    fn append_to(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        source_style_svg_bytes: usize,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        let neo_mask_svg_bytes = self
            .neo_edge_mask
            .map_or(0, FlowchartNeoEdgeMaskPlan::serialized_bytes);
        let ink_svg_bytes = self
            .adaptive_ink
            .map_or(0, |ink| "stroke:;".len() + ink.len());
        // Marker attributes are already owned by `FlowchartMarkerEmissionPlan`.
        let Some(projected_contribution) = source_style_svg_bytes
            .checked_add(neo_mask_svg_bytes)
            .and_then(|bytes| bytes.checked_add(ink_svg_bytes))
        else {
            work_meter.check_svg_append(usize::MAX, 1)?;
            unreachable!("overflowing Flowchart edge contribution must be rejected")
        };
        let neo_mask_work = self
            .neo_edge_mask
            .map_or(0, FlowchartNeoEdgeMaskPlan::work_units);
        work_meter.preflight(neo_mask_work)?;
        let rewritten_style = if self.line_hop_applied {
            work_meter.check_svg_append(0, projected_contribution)?;
            let mut style = String::with_capacity(projected_contribution);
            if let Some(mask) = self.neo_edge_mask {
                mask.write_to(&mut style)
                    .expect("writing into a String cannot fail");
            }
            self.write_source_style(&mut style)
                .expect("writing into a String cannot fail");
            Some(
                crate::svg::parity::line_hops::rewrite_style_after_line_hop(
                    &style, self.d, work_meter,
                )?
                .into_owned(),
            )
        } else {
            None
        };
        let projected_contribution = rewritten_style
            .as_ref()
            .map_or(projected_contribution, String::len);
        let serialized_bytes = self.serialized_bytes(projected_contribution, work_meter)?;

        // This is the only whole-edge boundary. Every byte from `<path` through `/>` is counted
        // against the current document before any part of the edge reaches the output buffer.
        work_meter.check_svg_append(out.len(), serialized_bytes)?;
        work_meter.charge_svg_bytes(projected_contribution)?;
        work_meter.charge(neo_mask_work)?;

        let before = out.len();
        if self.write_to(out, rewritten_style.as_deref()).is_err() {
            return match out.checkpoint() {
                Err(error) => Err(error),
                Ok(()) => Err(crate::Error::InvalidModel {
                    message: "prepared Flowchart edge writer failed without an output error"
                        .to_string(),
                }),
            };
        }
        out.checkpoint()?;
        debug_assert_eq!(out.len() - before, serialized_bytes);
        Ok(())
    }

    fn serialized_bytes(
        &self,
        projected_style_bytes: usize,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<usize> {
        let fixed_bytes = FIXED_PATH_BYTES
            + if self.hand_drawn {
                HAND_DRAWN_ATTRIBUTES.len()
            } else {
                0
            };
        let bytes = fixed_bytes
            .checked_add(self.d.len())
            .and_then(|bytes| bytes.checked_add(self.edge_dom_id.serialized_len()))
            .and_then(|bytes| bytes.checked_add(self.class_attr.len()))
            .and_then(|bytes| bytes.checked_add(projected_style_bytes))
            .and_then(|bytes| bytes.checked_add(escaped_xml_len(&self.edge.id)))
            .and_then(|bytes| bytes.checked_add(self.data_points_b64.len()))
            .and_then(|bytes| bytes.checked_add(escaped_xml_len(self.data_look.as_str())))
            .and_then(|bytes| bytes.checked_add(self.marker_attrs.len()));
        let Some(bytes) = bytes else {
            work_meter.check_svg_append(usize::MAX, 1)?;
            unreachable!("overflowing Flowchart edge serialization must be rejected")
        };
        Ok(bytes)
    }

    fn write_to(&self, out: &mut impl fmt::Write, rewritten_style: Option<&str>) -> fmt::Result {
        self.write_prefix_to(out)?;
        if let Some(style) = rewritten_style {
            out.write_str(style)?;
        } else {
            if let Some(mask) = self.neo_edge_mask {
                mask.write_to(out)?;
            }
            self.write_source_style(out)?;
        }
        self.write_suffix_to(out)
    }

    fn write_prefix_to(&self, out: &mut impl fmt::Write) -> fmt::Result {
        out.write_str(PATH_START)?;
        out.write_str(self.d)?;
        out.write_str(ID_ATTRIBUTE)?;
        write!(out, "{}", self.edge_dom_id)?;
        out.write_str(CLASS_ATTRIBUTE)?;
        out.write_str(self.class_attr)?;
        out.write_str(STYLE_ATTRIBUTE)
    }

    fn write_suffix_to(&self, out: &mut impl fmt::Write) -> fmt::Result {
        if self.hand_drawn {
            out.write_str(HAND_DRAWN_ATTRIBUTES)?;
        }
        out.write_str(DATA_ID_ATTRIBUTE)?;
        write!(out, "{}", escape_xml_display(&self.edge.id))?;
        out.write_str(DATA_POINTS_ATTRIBUTE)?;
        out.write_str(self.data_points_b64)?;
        out.write_str(DATA_LOOK_ATTRIBUTE)?;
        write!(out, "{}", escape_xml_display(self.data_look.as_str()))?;
        out.write_str(ATTRIBUTE_END)?;
        out.write_str(self.marker_attrs)?;
        out.write_str(PATH_END)
    }

    fn write_source_style(&self, out: &mut impl fmt::Write) -> fmt::Result {
        if let Some(ink) = self.adaptive_ink {
            write!(out, "stroke:{ink};")?;
        }
        if self.hand_drawn {
            return write_style_joined(out, self.default_edge_style, &self.edge.style);
        }

        for (index, declaration) in self
            .emitted_styles
            .emitted_edge_class_declarations()
            .iter()
            .enumerate()
        {
            if index != 0 {
                out.write_char(';')?;
            }
            write!(
                out,
                "{}:{}",
                escape_xml_display(declaration.property_css()),
                escape_xml_display(declaration.source_value())
            )?;
        }
        if !self
            .emitted_styles
            .emitted_edge_class_declarations()
            .is_empty()
        {
            out.write_char(';')?;
        }
        if self.default_edge_style.is_empty() && self.edge.style.is_empty() {
            out.write_char(';')
        } else {
            write_style_joined(out, self.default_edge_style, &self.edge.style)?;
            out.write_str(";;;")?;
            write_style_joined(out, self.default_edge_style, &self.edge.style)
        }
    }
}

fn write_style_joined(
    out: &mut impl fmt::Write,
    first: &[String],
    second: &[String],
) -> fmt::Result {
    for (index, part) in first.iter().chain(second).enumerate() {
        if index != 0 {
            out.write_char(';')?;
        }
        write!(out, "{}", escape_xml_display(part))?;
    }
    Ok(())
}

fn flowchart_neo_edge_path_length(
    geom: &FlowchartEdgePathGeom,
    edge: &crate::flowchart::FlowEdge,
) -> Option<f64> {
    if geom.line_hop_applied && flowchart_neo_mask_uses_dash_pairs(edge) {
        geom.pre_hop_dash_mask_length
    } else {
        svg_path_length_from_d(&geom.d)
    }
}

fn flowchart_neo_marker_mask_offset(arrow_type: Option<&str>) -> f64 {
    match arrow_type {
        Some("arrow_point" | "arrow_open_head") => 4.0,
        Some("arrow_hollow_circle") => 8.0,
        Some("arrow_cross" | "arrow_circle") => 12.5,
        _ => 0.0,
    }
}

#[derive(Debug, Clone, Copy)]
struct FlowchartNeoEdgeMaskPlan {
    start_offset: f64,
    middle_length: f64,
    end_offset: f64,
    repeated_dash_pairs: Option<usize>,
    serialized_bytes: usize,
}

impl FlowchartNeoEdgeMaskPlan {
    fn prepare(
        path_length: f64,
        edge: &crate::flowchart::FlowEdge,
        line_hop_applied: bool,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let (arrow_type_start, arrow_type_end) =
            super::super::edge_geom::arrow_types_for_edge(edge.edge_type.as_deref());
        let start_offset = flowchart_neo_marker_mask_offset(arrow_type_start);
        let end_offset = flowchart_neo_marker_mask_offset(arrow_type_end);
        let middle_length = if line_hop_applied {
            (path_length - start_offset - end_offset).max(0.0)
        } else {
            path_length - start_offset - end_offset
        };
        let repeated_dash_pairs = flowchart_neo_mask_uses_dash_pairs(edge)
            .then(|| (middle_length / 4.0).floor().max(0.0) as usize);

        let middle_bytes = if let Some(repeated_dash_pairs) = repeated_dash_pairs {
            NEO_EDGE_MASK_DASH_PAIR
                .len()
                .checked_mul(repeated_dash_pairs)
        } else {
            formatted_svg_number_bytes(middle_length).checked_add(1)
        };
        let serialized_bytes = NEO_EDGE_MASK_PREFIX
            .len()
            .checked_add(formatted_svg_number_bytes(start_offset))
            .and_then(|bytes| bytes.checked_add(1))
            .and_then(|bytes| bytes.checked_add(middle_bytes?))
            .and_then(|bytes| bytes.checked_add(formatted_svg_number_bytes(end_offset)))
            .and_then(|bytes| bytes.checked_add(NEO_EDGE_MASK_SUFFIX.len()));
        let Some(serialized_bytes) = serialized_bytes else {
            work_meter.check_svg_append(usize::MAX, 1)?;
            unreachable!("overflowing Neo edge mask projection must be rejected")
        };

        Ok(Self {
            start_offset,
            middle_length,
            end_offset,
            repeated_dash_pairs,
            serialized_bytes,
        })
    }

    const fn serialized_bytes(self) -> usize {
        self.serialized_bytes
    }

    fn work_units(self) -> usize {
        self.repeated_dash_pairs.unwrap_or(0)
    }

    fn write_to(self, out: &mut impl fmt::Write) -> fmt::Result {
        out.write_str(NEO_EDGE_MASK_PREFIX)?;
        write!(out, "{} ", fmt(self.start_offset))?;
        if let Some(repeated_dash_pairs) = self.repeated_dash_pairs {
            for _ in 0..repeated_dash_pairs {
                out.write_str(NEO_EDGE_MASK_DASH_PAIR)?;
            }
        } else {
            write!(out, "{} ", fmt(self.middle_length))?;
        }
        write!(out, "{}{}", fmt(self.end_offset), NEO_EDGE_MASK_SUFFIX)
    }
}

#[derive(Default)]
struct SvgNumberByteCounter(usize);

impl std::fmt::Write for SvgNumberByteCounter {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.0 = self.0.saturating_add(value.len());
        Ok(())
    }
}

fn formatted_svg_number_bytes(value: f64) -> usize {
    let mut counter = SvgNumberByteCounter::default();
    let _ = write!(&mut counter, "{}", fmt(value));
    counter.0
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
    fn neo_solid_mask_uses_original_length_and_marker_offsets() {
        let mut style = String::new();
        let plan = neo_mask(40.0, &edge("arrow_point", "normal"), true);
        plan.write_to(&mut style)
            .expect("write solid Neo edge mask");
        assert_eq!(style.len(), plan.serialized_bytes());
        assert_eq!(style, "stroke-dasharray: 0 0 36 4; stroke-dashoffset: 0;");

        style.clear();
        let plan = neo_mask(50.0, &edge("double_arrow_circle", "normal"), true);
        plan.write_to(&mut style)
            .expect("write double-ended Neo edge mask");
        assert_eq!(style.len(), plan.serialized_bytes());
        assert_eq!(
            style,
            "stroke-dasharray: 0 12.5 25 12.5; stroke-dashoffset: 0;"
        );
    }

    #[test]
    fn bpmn_marker_masks_preserve_the_pinned_open_arrow_and_hollow_circle_offsets() {
        for (edge_type, expected) in [
            (
                "arrow_open_head",
                "stroke-dasharray: 0 0 36 4; stroke-dashoffset: 0;",
            ),
            (
                "arrow_hollow_circle",
                "stroke-dasharray: 0 0 32 8; stroke-dashoffset: 0;",
            ),
        ] {
            let plan = neo_mask(40.0, &edge(edge_type, "normal"), true);
            let mut style = String::new();
            plan.write_to(&mut style).unwrap();
            assert_eq!(style, expected);
            assert_eq!(style.len(), plan.serialized_bytes());
        }
    }

    #[test]
    fn neo_dotted_mask_preserves_upstream_two_pixel_pattern() {
        let mut style = String::new();
        let plan = neo_mask(16.0, &edge("arrow_open", "dotted"), false);
        plan.write_to(&mut style)
            .expect("write dotted Neo edge mask");
        assert_eq!(style.len(), plan.serialized_bytes());
        assert_eq!(
            style,
            "stroke-dasharray: 0 0 2 2 2 2 2 2 2 2 0; stroke-dashoffset: 0;"
        );
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
            .expect("finite source-style projection");
        let neo_edge_mask = neo_mask(16.0, &edge, false);
        let key = crate::flowchart::FlowchartEdgeKey::self_loop_segment(12, 2);
        let document_ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
            diagram_id: "diagram",
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
            emitted_styles: &emitted_styles,

            class_attr: "edge-thickness-normal edge-pattern-dotted edge-thickness-normal edge-pattern-solid flowchart-link",
            marker_attrs: r#" marker-start="url(#diagram-flowchart-v2-circleStart)" marker-end="url(#diagram-flowchart-v2-pointEnd)""#,
            default_edge_style: &default_edge_style,
            adaptive_ink: None,
            neo_edge_mask: Some(neo_edge_mask),
            line_hop_applied: false,
        };
        let expected_edge = r##"<path d="M0,0L16,0" id="diagram-merman-flowchart-document-edge-12-segment-2" class="edge-thickness-normal edge-pattern-dotted edge-thickness-normal edge-pattern-solid flowchart-link" style="stroke-dasharray: 0 0 2 2 2 2 2 2 2 2 0; stroke-dashoffset: 0;stroke:#2563eb;opacity:0.5;stroke:#ef4444;;;opacity:0.5;stroke:#ef4444" data-edge="true" data-et="edge" data-id="边&quot;&amp;&lt;]]&gt;" data-points="W3sieCI6MH1d" data-look="neo" marker-start="url(#diagram-flowchart-v2-circleStart)" marker-end="url(#diagram-flowchart-v2-pointEnd)" />"##;
        let initial = "prefix";
        let probe_meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        assert_eq!(
            emission
                .serialized_bytes(
                    source_style_svg_bytes + neo_edge_mask.serialized_bytes(),
                    &probe_meter,
                )
                .expect("measure complete edge"),
            expected_edge.len()
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
            .unwrap();
        let mut hand_drawn_svg = String::new();
        hand_drawn
            .append_to(&mut hand_drawn_svg, hand_drawn_style_bytes, &probe_meter)
            .unwrap();
        assert!(hand_drawn_svg.contains(
            r##"style="opacity:0.5;stroke:#ef4444" stroke="#000" stroke-width="1" fill="none""##
        ));
        assert_eq!(
            hand_drawn
                .serialized_bytes(hand_drawn_style_bytes, &probe_meter)
                .unwrap(),
            hand_drawn_svg.len()
        );

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
        assert_eq!(interrupted.as_str(), format!("{initial}{PATH_START}"));
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

    #[test]
    fn neo_dotted_mask_projection_rejects_arithmetic_overflow_without_looping() {
        let edge = edge("arrow_open", "dotted");
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let error = FlowchartNeoEdgeMaskPlan::prepare(f64::MAX, &edge, false, &meter)
            .expect_err("overflowing mask projection must fail in constant work");
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured SVG arithmetic rejection");
        };
        assert_eq!(error.limit, "max_svg_bytes");
        assert_eq!(
            error.cause,
            crate::resources::ResourceLimitCause::ArithmeticOverflow
        );
        assert_eq!(meter.used(), 0);
        assert_eq!(meter.projected_svg_bytes(), 0);
    }
}
