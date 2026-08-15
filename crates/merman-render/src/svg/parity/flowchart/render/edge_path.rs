//! Flowchart edge path renderer.

use super::super::defs::FlowchartMarkerEmissionPlan;
use super::super::*;
use std::fmt;

const NEO_EDGE_MASK_PREFIX: &str = "stroke-dasharray: 0 ";
const NEO_EDGE_MASK_DASH_PAIR: &str = "2 2 ";
const NEO_EDGE_MASK_SUFFIX: &str = "; stroke-dashoffset: 0;";

pub(in crate::svg::parity::flowchart) fn render_flowchart_edge_path(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    hierarchy_plan: &FlowchartHierarchyPlan<'_>,
    marker_plan: &FlowchartMarkerEmissionPlan,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    origin_x: f64,
    origin_y: f64,
    scratch: &mut FlowchartEdgeDataPointsScratch,
    edge_cache: &mut FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
) -> crate::Result<()> {
    let key = edge_ref.key;
    let edge = edge_ref.edge;
    let trace_enabled = ctx.trace_edge_id.is_some_and(|id| id == edge.id.as_str());
    let data_look = flowchart_config_diagram_look(ctx.config);
    let hand_drawn = data_look.is_hand_drawn();
    let emitted_styles = ctx.edge_style_plan.edge_for(key)?;
    let animation = ctx.edge_style_plan.animation_for(key)?;
    let stroke_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        emitted_styles.emitted_edge_source_stroke_status(hand_drawn),
        ctx.edge_stroke_config_override,
    );
    let typed_stroke = ctx.edge_theme.stroke_value(stroke_precedence, true);
    let stroke_dasharray_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        emitted_styles.emitted_edge_source_stroke_dasharray_status(hand_drawn),
        false,
    );
    let edge_writer_supports_typed_dasharray = !hand_drawn;
    let typed_stroke_dasharray = ctx.edge_theme.stroke_dasharray_value(
        stroke_dasharray_precedence,
        edge_writer_supports_typed_dasharray,
    );

    let cached_geom = edge_cache
        .get(&key)
        .filter(|c| (c.origin_x - origin_x).abs() <= 1e-9 && (c.origin_y - origin_y).abs() <= 1e-9)
        .map(|c| &c.geom);

    // Trace collection recomputes the pre-line-hop geometry for diagnostics, but the emitted SVG
    // must still consume the post-processed cache. Enabling diagnostics must not alter rendering.
    let owned_geom = if cached_geom.is_none() || trace_enabled {
        flowchart_compute_edge_path_geom(
            FlowchartEdgePathGeomRequest {
                ctx,
                key,
                edge,
                origin_x,
                origin_y,
                trace_enabled,
                collapse_degenerate_subgraph_route: hierarchy_plan
                    .is_degenerate_subgraph_descendant_edge(ctx, edge),
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
            let source_residuals =
                emitted_styles.emitted_shape_source_residuals(edge.id.as_str(), false);
            if ctx.resolved_theme.is_some() || !source_residuals.is_empty() {
                ctx.theme_evidence.record_edge_emission(
                    &ctx.edge_theme,
                    crate::flowchart::FlowchartEdgeThemeEmission {
                        stroke: crate::flowchart::FlowchartThemeFacetEmission::new(
                            stroke_precedence,
                            false,
                        ),
                        stroke_dasharray: crate::flowchart::FlowchartThemeFacetEmission::new(
                            stroke_dasharray_precedence,
                            false,
                        ),
                    },
                    &source_residuals,
                );
            }
            return Ok(());
        };
        g
    };
    let d = geom.d.as_str();
    let data_points_b64 = geom.data_points_b64.as_str();
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

    scratch.edge_class_attr.clear();
    css::write_flowchart_edge_class_attr(&mut scratch.edge_class_attr, edge, animation);
    if hand_drawn {
        scratch.edge_class_attr.push_str(" transition");
    }
    scratch.edge_marker_attrs.clear();
    marker_plan.push_edge_marker_attributes_for(
        &mut scratch.edge_marker_attrs,
        ctx.diagram_id,
        ctx.diagram_type,
        key,
        ctx.work_meter,
    )?;
    let source_style_svg_bytes = ctx.edge_style_plan.edge_source_style_svg_bytes_for(key)?;
    FlowchartEdgeSvgEmission {
        diagram_id: ctx.diagram_id,
        edge,
        d,
        data_points_b64,
        data_look,
        hand_drawn,
        emitted_styles,
        typed_stroke,
        typed_stroke_dasharray,
        class_attr: &scratch.edge_class_attr,
        marker_attrs: &scratch.edge_marker_attrs,
        default_edge_style: &ctx.default_edge_style,
        neo_edge_mask,
    }
    .append_to(out, source_style_svg_bytes, ctx.work_meter)?;

    let source_residuals =
        emitted_styles.emitted_edge_source_residuals(edge.id.as_str(), hand_drawn);
    if ctx.resolved_theme.is_some() || !source_residuals.is_empty() {
        ctx.theme_evidence.record_edge_emission(
            &ctx.edge_theme,
            crate::flowchart::FlowchartEdgeThemeEmission {
                stroke: crate::flowchart::FlowchartThemeFacetEmission::new(
                    stroke_precedence,
                    typed_stroke.is_some(),
                ),
                stroke_dasharray: crate::flowchart::FlowchartThemeFacetEmission::new(
                    stroke_dasharray_precedence,
                    typed_stroke_dasharray.is_some(),
                ),
            },
            &source_residuals,
        );
    }
    if let Some(emitted_d_for_label) = rough_d
        && let Some(cache_entry) = edge_cache.get_mut(&key)
        && (cache_entry.origin_x - origin_x).abs() <= 1e-9
        && (cache_entry.origin_y - origin_y).abs() <= 1e-9
    {
        cache_entry.geom.emitted_d_for_label = Some(emitted_d_for_label);
    }
    Ok(())
}

struct FlowchartEdgeSvgEmission<'a> {
    diagram_id: &'a str,
    edge: &'a crate::flowchart::FlowEdge,
    d: &'a str,
    data_points_b64: &'a str,
    data_look: crate::config::DiagramLook<'a>,
    hand_drawn: bool,
    emitted_styles: &'a FlowchartCompiledStyles,
    typed_stroke: Option<&'a str>,
    typed_stroke_dasharray: Option<&'a str>,
    class_attr: &'a str,
    marker_attrs: &'a str,
    default_edge_style: &'a [String],
    neo_edge_mask: Option<FlowchartNeoEdgeMaskPlan>,
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
        // Marker attributes are already owned by `FlowchartMarkerEmissionPlan`; only the repeated
        // source-style payload and the Neo mask remain disjoint producer contributions here.
        let Some(projected_contribution) = source_style_svg_bytes.checked_add(neo_mask_svg_bytes)
        else {
            work_meter.check_svg_append(usize::MAX, 1)?;
            unreachable!("overflowing Flowchart edge contribution must be rejected")
        };
        let serialized_bytes = self.serialized_bytes(projected_contribution, work_meter)?;
        let neo_mask_work = self
            .neo_edge_mask
            .map_or(0, FlowchartNeoEdgeMaskPlan::work_units);

        // This is the only whole-edge boundary. Every byte from `<path` through `/>` is counted
        // against the current document before any part of the edge reaches the output buffer.
        work_meter.check_svg_append(out.len(), serialized_bytes)?;
        work_meter.preflight(neo_mask_work)?;
        work_meter.charge_svg_bytes(projected_contribution)?;
        work_meter.charge(neo_mask_work)?;

        let before = out.len();
        if self.write_to(out).is_err() {
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
        let mut counter = FlowchartEdgeSvgByteCounter::default();
        let result = self
            .write_prefix_to(&mut counter)
            .and_then(|()| counter.add_bytes(projected_style_bytes))
            .and_then(|()| self.write_suffix_to(&mut counter));
        debug_assert!(result.is_ok() || counter.overflowed);
        counter.finish(work_meter)
    }

    fn write_to(&self, out: &mut impl fmt::Write) -> fmt::Result {
        self.write_prefix_to(out)?;
        if let Some(mask) = self.neo_edge_mask {
            mask.write_to(out)?;
        }
        self.write_source_style(out)?;
        self.write_suffix_to(out)
    }

    fn write_prefix_to(&self, out: &mut impl fmt::Write) -> fmt::Result {
        write!(
            out,
            r#"<path d="{}" id="{}-{}" class="{}" style=""#,
            self.d,
            escape_xml_display(self.diagram_id),
            escape_xml_display(&self.edge.id),
            self.class_attr,
        )
    }

    fn write_suffix_to(&self, out: &mut impl fmt::Write) -> fmt::Result {
        if let Some(stroke) = self.typed_stroke {
            out.write_str(";stroke:")?;
            write!(out, "{}", escape_xml_display(stroke))?;
            out.write_str(" !important")?;
        }
        if let Some(stroke_dasharray) = self.typed_stroke_dasharray {
            out.write_str(";stroke-dasharray:")?;
            // Typed dasharrays are canonical finite-number sequences separated by ASCII spaces.
            out.write_str(stroke_dasharray)?;
            out.write_str(" !important")?;
        }
        if self.hand_drawn {
            out.write_str(r##"" stroke="#000" stroke-width="1" fill="none"##)?;
        }
        write!(
            out,
            r#"" data-edge="true" data-et="edge" data-id="{}" data-points="{}" data-look="{}""#,
            escape_xml_display(&self.edge.id),
            self.data_points_b64,
            escape_xml_display(self.data_look.as_str()),
        )?;
        out.write_str(self.marker_attrs)?;
        out.write_str(" />")
    }

    fn write_source_style(&self, out: &mut impl fmt::Write) -> fmt::Result {
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

#[derive(Default)]
struct FlowchartEdgeSvgByteCounter {
    bytes: usize,
    overflowed: bool,
}

impl FlowchartEdgeSvgByteCounter {
    fn add_bytes(&mut self, additional: usize) -> fmt::Result {
        let Some(bytes) = self.bytes.checked_add(additional) else {
            self.overflowed = true;
            return Err(fmt::Error);
        };
        self.bytes = bytes;
        Ok(())
    }

    fn finish(self, work_meter: &crate::resources::OperationWorkMeter) -> crate::Result<usize> {
        if self.overflowed {
            work_meter.check_svg_append(usize::MAX, 1)?;
            unreachable!("overflowing Flowchart edge serialization must be rejected")
        }
        Ok(self.bytes)
    }
}

impl fmt::Write for FlowchartEdgeSvgByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.add_bytes(value.len())
    }
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
        let repeated_dash_pairs = matches!(edge.stroke.as_deref(), Some("dotted" | "dashed"))
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
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};

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
    fn neo_solid_mask_uses_final_line_hop_length_and_marker_offsets() {
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
        edge.classes.push("accent".to_string());
        edge.style.push("stroke:#ef4444".to_string());
        let emitted_styles =
            flowchart_compile_styles(&class_defs, &edge.classes, &default_edge_style, &edge.style)
                .into_edge_artifact(false);
        let source_style_svg_bytes = emitted_styles
            .projected_edge_source_style_bytes(&default_edge_style, &edge.style, false)
            .expect("finite source-style projection");
        let neo_edge_mask = neo_mask(16.0, &edge, false);
        let emission = FlowchartEdgeSvgEmission {
            diagram_id: "diagram<&",
            edge: &edge,
            d: "M0,0L16,0",
            data_points_b64: "W3sieCI6MH1d",
            data_look: crate::config::DiagramLook::from_raw(Some("neo")),
            hand_drawn: false,
            emitted_styles: &emitted_styles,
            typed_stroke: Some("#0f172a"),
            typed_stroke_dasharray: Some("7 3"),
            class_attr: "edge-thickness-normal edge-pattern-dotted edge-thickness-normal edge-pattern-solid flowchart-link",
            marker_attrs: r#" marker-start="url(#diagram-flowchart-v2-circleStart)" marker-end="url(#diagram-flowchart-v2-pointEnd)""#,
            default_edge_style: &default_edge_style,
            neo_edge_mask: Some(neo_edge_mask),
        };
        let expected_edge = r##"<path d="M0,0L16,0" id="diagram&lt;&amp;-edge" class="edge-thickness-normal edge-pattern-dotted edge-thickness-normal edge-pattern-solid flowchart-link" style="stroke-dasharray: 0 0 2 2 2 2 2 2 2 2 0; stroke-dashoffset: 0;stroke:#2563eb;opacity:0.5;stroke:#ef4444;;;opacity:0.5;stroke:#ef4444;stroke:#0f172a !important;stroke-dasharray:7 3 !important" data-edge="true" data-et="edge" data-id="edge" data-points="W3sieCI6MH1d" data-look="neo" marker-start="url(#diagram-flowchart-v2-circleStart)" marker-end="url(#diagram-flowchart-v2-pointEnd)" />"##;
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

        let exact_meter = meter_with_limits(initial.len() + expected_edge.len(), 4);
        let mut exact = initial.to_string();
        emission
            .append_to(&mut exact, source_style_svg_bytes, &exact_meter)
            .expect("exact whole-edge budgets must pass");
        assert_eq!(exact, format!("{initial}{expected_edge}"));
        assert_eq!(exact_meter.used(), 4);
        assert_eq!(
            exact_meter.projected_svg_bytes(),
            source_style_svg_bytes + neo_edge_mask.serialized_bytes()
        );

        let short_svg_meter = meter_with_limits(initial.len() + expected_edge.len() - 1, 4);
        let mut short_svg = initial.to_string();
        let error = emission
            .append_to(&mut short_svg, source_style_svg_bytes, &short_svg_meter)
            .expect_err("short SVG budget must reject before path emission");
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured SVG resource rejection");
        };
        assert_eq!(error.limit, "max_svg_bytes");
        assert_eq!(error.actual, initial.len() + expected_edge.len());
        assert_eq!(short_svg, initial);
        assert_eq!(short_svg_meter.used(), 0);
        assert_eq!(short_svg_meter.projected_svg_bytes(), 0);

        let short_work_meter = meter_with_limits(initial.len() + expected_edge.len(), 3);
        let mut short_work = initial.to_string();
        let error = emission
            .append_to(&mut short_work, source_style_svg_bytes, &short_work_meter)
            .expect_err("short work budget must reject before path emission");
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured work resource rejection");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(short_work, initial);
        assert_eq!(short_work_meter.used(), 0);
        assert_eq!(short_work_meter.projected_svg_bytes(), 0);
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
