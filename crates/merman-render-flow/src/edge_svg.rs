//! Complete Flow SVG edge paths on the bounded output sink.

use std::fmt::Write as _;
use std::{fmt, sync::Arc};

use base64::Engine as _;
use merman_model::families::flowchart::FlowEdge;
use merman_render_core::{
    Error, LayoutPoint, Result,
    config::DiagramLook,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    svg_edge_parity::path_numbers::json_stringify_points_into,
    svg_edge_parity::{edge_path::neo_marker_mask_offset, line_hops::rewrite_style_after_line_hop},
    svg_output::SvgOutput,
    xml::{escape_attr_display, escaped_attr_len},
};

use crate::{document::FlowchartDocumentId, source_style::FlowchartStyleDeclaration};

mod step;
mod terminal;

pub use step::FlowchartSteps;
pub use terminal::{
    FlowchartEdgeSvgContext, FlowchartEdgeSvgGeometry, FlowchartEdgeSvgScratch,
    FlowchartEdgeTerminal,
};

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

pub fn arrow_types_for_edge(
    edge_type: Option<&str>,
) -> (Option<&'static str>, Option<&'static str>) {
    let arrow_type_start = match edge_type {
        Some("double_arrow_point") => Some("arrow_point"),
        Some("double_arrow_circle") => Some("arrow_circle"),
        Some("double_arrow_cross") => Some("arrow_cross"),
        _ => None,
    };
    let arrow_type_end = match edge_type {
        Some("arrow_open") => None,
        Some("arrow_cross") => Some("arrow_cross"),
        Some("arrow_circle") => Some("arrow_circle"),
        Some("arrow_open_head") => Some("arrow_open_head"),
        Some("arrow_hollow_circle") => Some("arrow_hollow_circle"),
        Some("double_arrow_point" | "arrow_point") => Some("arrow_point"),
        Some("double_arrow_circle") => Some("arrow_circle"),
        Some("double_arrow_cross") => Some("arrow_cross"),
        _ => Some("arrow_point"),
    };

    (arrow_type_start, arrow_type_end)
}

pub fn flowchart_neo_mask_uses_dash_pairs(edge: &FlowEdge) -> bool {
    matches!(edge.stroke.as_deref(), Some("dotted" | "dashed"))
}

/// Reusable storage for Mermaid's serialized edge point attribute.
#[derive(Default)]
struct FlowchartEdgeDataPoints {
    json: String,
    base64: String,
    number: ryu_js::Buffer,
}

impl FlowchartEdgeDataPoints {
    fn encode(&mut self, points: &[LayoutPoint]) -> &str {
        self.json.clear();
        json_stringify_points_into(&mut self.json, points, &mut self.number);
        self.base64.clear();
        base64::engine::general_purpose::STANDARD
            .encode_string(self.json.as_bytes(), &mut self.base64);
        &self.base64
    }
}

struct FlowchartEdgeSvgEmission<'a> {
    edge_dom_id: FlowchartDocumentId<'a>,
    edge: &'a FlowEdge,
    d: &'a str,
    data_points_b64: &'a str,
    data_look: DiagramLook<'a>,
    hand_drawn: bool,
    class_declarations: &'a [Arc<FlowchartStyleDeclaration>],
    class_attr: &'a str,
    marker_attrs: &'a str,
    default_edge_style: &'a [String],
    repeat_inline: bool,
    adaptive_ink: Option<merman_theme_contract::v2::Color>,
    neo_edge_mask: Option<FlowchartNeoEdgeMaskPlan>,
    line_hop_applied: bool,
}

impl FlowchartEdgeSvgEmission<'_> {
    fn append_to(
        &self,
        out: &mut impl SvgOutput,
        source_style_svg_bytes: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Option<String>> {
        let neo_mask_svg_bytes = self
            .neo_edge_mask
            .map_or(0, FlowchartNeoEdgeMaskPlan::serialized_bytes);
        let ink_svg_bytes = self.adaptive_ink.map_or(0, |ink| {
            let alpha_bytes = 2 * usize::from(ink.channels()[3] != u8::MAX);
            ";stroke:#000000!important;".len() + alpha_bytes
        });
        // The caller supplies the already formatted marker attributes.
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
            self.write_paint_style(&mut style, None)
                .expect("writing into a String cannot fail");
            Some(rewrite_style_after_line_hop(&style, self.d, work_meter)?.into_owned())
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
                Ok(()) => Err(Error::InvalidModel {
                    message: "prepared Flowchart edge writer failed without an output error"
                        .to_string(),
                }),
            };
        }
        out.checkpoint()?;
        debug_assert_eq!(out.len() - before, serialized_bytes);
        Ok(rewritten_style)
    }

    /// Write the recolored shaft inside the caller's surface clip, leaving room for its markers.
    fn write_surface_path(
        &self,
        out: &mut impl SvgOutput,
        ink: &impl fmt::Display,
        rewritten_style: Option<&str>,
    ) -> Result<()> {
        fn write_path(
            path: &FlowchartEdgeSvgEmission<'_>,
            out: &mut impl fmt::Write,
            ink: &impl fmt::Display,
            rewritten_style: Option<&str>,
        ) -> fmt::Result {
            write!(
                out,
                r#"<path d="{}" class="{}" data-look="{}" style=""#,
                path.d,
                path.class_attr,
                escape_attr_display(path.data_look.as_str()),
            )?;
            if let Some(style) = rewritten_style {
                write!(out, "{style};stroke:{ink}!important;")?;
            } else {
                path.write_paint_style(out, Some(ink))?;
            }
            if path.hand_drawn {
                out.write_str(HAND_DRAWN_ATTRIBUTES)?;
                out.write_char('"')
            } else {
                out.write_str(r#"" fill="none""#)
            }
        }
        if write_path(self, out, ink, rewritten_style).is_err() {
            return out.checkpoint().and(Err(Error::InvalidModel {
                message: "prepared Flowchart surface writer failed without an output error"
                    .to_string(),
            }));
        }
        out.checkpoint()
    }

    fn serialized_bytes(
        &self,
        projected_style_bytes: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<usize> {
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
            .and_then(|bytes| bytes.checked_add(escaped_attr_len(&self.edge.id)))
            .and_then(|bytes| bytes.checked_add(self.data_points_b64.len()))
            .and_then(|bytes| bytes.checked_add(escaped_attr_len(self.data_look.as_str())))
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
            self.write_paint_style(out, None)?;
        }
        self.write_suffix_to(out)
    }

    fn write_paint_style(
        &self,
        out: &mut impl fmt::Write,
        layer_ink: Option<&dyn fmt::Display>,
    ) -> fmt::Result {
        if let Some(mask) = self.neo_edge_mask {
            mask.write_to(out)?;
        }
        self.write_source_style(out)?;
        if let Some(ink) = layer_ink {
            write!(out, ";stroke:{ink}!important;")?;
        } else if let Some(ink) = self.adaptive_ink {
            write!(out, ";stroke:{ink}!important;")?;
        }
        Ok(())
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
        write!(out, "{}", escape_attr_display(&self.edge.id))?;
        out.write_str(DATA_POINTS_ATTRIBUTE)?;
        out.write_str(self.data_points_b64)?;
        out.write_str(DATA_LOOK_ATTRIBUTE)?;
        write!(out, "{}", escape_attr_display(self.data_look.as_str()))?;
        out.write_str(ATTRIBUTE_END)?;
        out.write_str(self.marker_attrs)?;
        out.write_str(PATH_END)
    }

    fn write_source_style(&self, out: &mut impl fmt::Write) -> fmt::Result {
        if self.hand_drawn {
            return write_style_joined(out, self.default_edge_style, &self.edge.style);
        }

        for (index, declaration) in self.class_declarations.iter().enumerate() {
            if index != 0 {
                out.write_char(';')?;
            }
            write!(
                out,
                "{}:{}",
                escape_attr_display(declaration.property_css()),
                escape_attr_display(declaration.source_value())
            )?;
        }
        if !self.class_declarations.is_empty() {
            out.write_char(';')?;
        }
        write_style_joined(out, self.default_edge_style, &self.edge.style)?;
        if !self.repeat_inline {
            Ok(())
        } else if self.default_edge_style.is_empty() && self.edge.style.is_empty() {
            out.write_char(';')
        } else {
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
        write!(out, "{}", escape_attr_display(part))?;
    }
    Ok(())
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
        edge: &FlowEdge,
        line_hop_applied: bool,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self> {
        let (arrow_type_start, arrow_type_end) = arrow_types_for_edge(edge.edge_type.as_deref());
        let start_offset = neo_marker_mask_offset(arrow_type_start);
        let end_offset = neo_marker_mask_offset(arrow_type_end);
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
    use crate::{
        document::{FlowchartDocumentIdRequest, FlowchartDocumentIds},
        projection::{FlowchartEdgeKey, FlowchartRenderEdgeRef},
        style::compiled::flowchart_compile_styles,
    };
    use indexmap::IndexMap;
    use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};
    use merman_render_core::{
        resources::{
            RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
        },
        svg_css::SvgId,
        svg_output::BoundedSvgOutput,
    };
    use rustc_hash::FxHashMap;

    fn edge(edge_type: &str, stroke: &str) -> FlowEdge {
        FlowEdge {
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

    fn neo_mask(
        path_length: f64,
        edge: &FlowEdge,
        line_hop_applied: bool,
    ) -> FlowchartNeoEdgeMaskPlan {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
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
    fn neo_dotted_mask_projection_rejects_arithmetic_overflow_without_looping() {
        let edge = edge("arrow_open", "dotted");
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let error = FlowchartNeoEdgeMaskPlan::prepare(f64::MAX, &edge, false, &meter)
            .expect_err("overflowing mask projection must fail in constant work");
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured SVG arithmetic rejection");
        };
        assert_eq!(error.limit, "max_svg_bytes");
        assert_eq!(error.cause, ResourceLimitCause::ArithmeticOverflow);
        assert_eq!(meter.used(), 0);
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    fn meter_with_limits(svg_bytes: usize, work_units: usize) -> OperationWorkMeter {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, svg_bytes)
            .unwrap()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, work_units)
            .unwrap();
        OperationWorkMeter::new(policy)
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
        let key = FlowchartEdgeKey::self_loop_segment(12, 2);
        let document_ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
            diagram_id: SvgId::new("diagram").unwrap(),
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
            data_look: DiagramLook::from_raw(Some("neo")),
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
        let expected_edge = r##"<path d="M0,0L16,0" id="diagram-merman-flowchart-document-edge-12-segment-2" class="edge-thickness-normal edge-pattern-dotted edge-thickness-normal edge-pattern-solid flowchart-link" style="stroke-dasharray: 0 0 2 2 2 2 2 2 2 2 0; stroke-dashoffset: 0;stroke:#2563eb;opacity:0.5;stroke:#ef4444;;;opacity:0.5;stroke:#ef4444" data-edge="true" data-et="edge" data-id="边#quot;&amp;&lt;]]>" data-points="W3sieCI6MH1d" data-look="neo" marker-start="url(#diagram-flowchart-v2-circleStart)" marker-end="url(#diagram-flowchart-v2-pointEnd)" />"##;
        let initial = "prefix";
        let probe_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
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
            let Error::ResourceLimitExceeded(error) = error else {
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
        let Error::ResourceLimitExceeded(error) = error else {
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
        let Error::ResourceLimitExceeded(error) = error else {
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
            data_look: DiagramLook::from_raw(Some("handDrawn")),
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
        let Error::ResourceLimitExceeded(error) = error else {
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
        assert!(matches!(error, Error::Cancelled(details)
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
        let Error::Cancelled(cancellation) = error else {
            panic!("expected structured cancellation from the bounded writer");
        };
        assert_eq!(cancellation.phase, merman_model::OperationPhase::Emit);
        assert_eq!(interrupted.as_str(), format!(r#"{initial}<path d=""#));
        assert_eq!(interrupted_meter.used(), 4);
        assert_eq!(
            interrupted_meter.projected_svg_bytes(),
            projected_style_bytes
        );
        assert!(matches!(interrupted.finish(), Err(Error::Cancelled(error))
            if error == cancellation));
    }
}
