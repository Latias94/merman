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
    xml::{escape_attr_display, escape_xml_display, escaped_xml_len},
};

use crate::{document::FlowchartDocumentId, source_style::FlowchartStyleDeclaration};

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
pub struct FlowchartEdgeDataPoints {
    json: String,
    base64: String,
    number: ryu_js::Buffer,
}

impl FlowchartEdgeDataPoints {
    pub fn encode(&mut self, points: &[LayoutPoint]) -> &str {
        self.json.clear();
        json_stringify_points_into(&mut self.json, points, &mut self.number);
        self.base64.clear();
        base64::engine::general_purpose::STANDARD
            .encode_string(self.json.as_bytes(), &mut self.base64);
        &self.base64
    }
}

pub struct FlowchartEdgeSvgEmission<'a> {
    pub edge_dom_id: FlowchartDocumentId<'a>,
    pub edge: &'a FlowEdge,
    pub d: &'a str,
    pub data_points_b64: &'a str,
    pub data_look: DiagramLook<'a>,
    pub hand_drawn: bool,
    pub class_declarations: &'a [Arc<FlowchartStyleDeclaration>],
    pub class_attr: &'a str,
    pub marker_attrs: &'a str,
    pub default_edge_style: &'a [String],
    pub adaptive_ink: Option<&'static str>,
    pub neo_edge_mask: Option<FlowchartNeoEdgeMaskPlan>,
    pub line_hop_applied: bool,
}

impl FlowchartEdgeSvgEmission<'_> {
    pub fn append_to(
        &self,
        out: &mut impl SvgOutput,
        source_style_svg_bytes: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        let neo_mask_svg_bytes = self
            .neo_edge_mask
            .map_or(0, FlowchartNeoEdgeMaskPlan::serialized_bytes);
        let ink_svg_bytes = self
            .adaptive_ink
            .map_or(0, |ink| "stroke:;".len() + ink.len());
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
            if let Some(mask) = self.neo_edge_mask {
                mask.write_to(&mut style)
                    .expect("writing into a String cannot fail");
            }
            self.write_source_style(&mut style)
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
        Ok(())
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
        write!(out, "{}", escape_attr_display(part))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct FlowchartNeoEdgeMaskPlan {
    start_offset: f64,
    middle_length: f64,
    end_offset: f64,
    repeated_dash_pairs: Option<usize>,
    serialized_bytes: usize,
}

impl FlowchartNeoEdgeMaskPlan {
    pub fn prepare(
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

    pub const fn serialized_bytes(self) -> usize {
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
    use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};
    use merman_render_core::resources::{RenderResourcePolicy, ResourceLimitCause};

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
}
