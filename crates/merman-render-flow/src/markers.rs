//! Mermaid base and colored marker definitions shared by the three Flow SVG producers.

use std::fmt;

use merman_render_core::{
    svg_edge_parity::markers::{BaseEdgeMarkerKind, hierarchy_edge_marker_body},
    theme_svg::marker::arrow_path,
    xml::{escape_xml, escape_xml_display},
};
use merman_theme_contract::v2::MarkerStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowchartMarkerBase {
    PointStart,
    PointEnd,
    CircleStart,
    CircleEnd,
    CrossStart,
    CrossEnd,
    OpenArrowStart,
    OpenArrowEnd,
    HollowCircleStart,
    HollowCircleEnd,
}

impl FlowchartMarkerBase {
    pub const fn id_suffix(self) -> &'static str {
        match self {
            Self::PointStart => "pointStart",
            Self::PointEnd => "pointEnd",
            Self::CircleStart => "circleStart",
            Self::CircleEnd => "circleEnd",
            Self::CrossStart => "crossStart",
            Self::CrossEnd => "crossEnd",
            Self::OpenArrowStart => "openArrowStart",
            Self::OpenArrowEnd => "openArrowEnd",
            Self::HollowCircleStart => "hollowCircleStart",
            Self::HollowCircleEnd => "hollowCircleEnd",
        }
    }

    pub const fn supports_margin(self) -> bool {
        !matches!(
            self,
            Self::OpenArrowStart
                | Self::OpenArrowEnd
                | Self::HollowCircleStart
                | Self::HollowCircleEnd
        )
    }
}

const AGENTFLOW_HIERARCHY_MARKERS: [(&str, &str); 2] = [
    ("hierarchyEnd", hierarchy_edge_marker_body(false)),
    ("hierarchyStart", hierarchy_edge_marker_body(true)),
];

#[derive(Debug, Clone, Copy)]
enum FlowchartMarkerPaint {
    None,
    Stroke,
    StrokeAndFill,
}

impl FlowchartMarkerPaint {
    fn color_scan_count(self) -> usize {
        match self {
            Self::None | Self::Stroke => 1,
            Self::StrokeAndFill => 2,
        }
    }

    fn push(self, out: &mut impl fmt::Write, color: &str) {
        match self {
            Self::None => {}
            Self::Stroke => {
                let _ = write!(out, r#" stroke="{}""#, escape_xml_display(color));
            }
            Self::StrokeAndFill => {
                let color = escape_xml_display(color);
                let _ = write!(out, r#" stroke="{color}" fill="{color}""#);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FlowchartMarkerShapeSpec {
    base: FlowchartMarkerBase,
    margin: bool,
    marker_class: &'static str,
    shape: &'static str,
    paint: FlowchartMarkerPaint,
    pub viewport: FlowchartMarkerViewport,
}

#[derive(Debug, Clone, Copy)]
pub struct FlowchartMarkerViewport {
    pub view_box: [f64; 2],
    pub reference: [f64; 2],
    pub size: [f64; 2],
}

impl FlowchartMarkerViewport {
    pub fn rotation_radius(self) -> f64 {
        // All built-in markers use a zero-origin viewBox and the SVG default
        // xMidYMid meet mapping. The default hidden overflow clips to this viewport.
        let scale = (self.size[0] / self.view_box[0]).min(self.size[1] / self.view_box[1]);
        let reference_x =
            self.reference[0] * scale + (self.size[0] - self.view_box[0] * scale) / 2.0;
        let reference_y =
            self.reference[1] * scale + (self.size[1] - self.view_box[1] * scale) / 2.0;
        // The farthest viewport corner bounds the marker under any path tangent rotation.
        reference_x
            .abs()
            .max((self.size[0] - reference_x).abs())
            .hypot(reference_y.abs().max((self.size[1] - reference_y).abs()))
    }
}

fn write_marker_shape(
    out: &mut impl fmt::Write,
    spec: &FlowchartMarkerShapeSpec,
    style: MarkerStyle,
) {
    use merman_render_core::number_format::canonical_number as number;

    let start = match spec.base {
        FlowchartMarkerBase::PointStart => Some(true),
        FlowchartMarkerBase::PointEnd => Some(false),
        _ => None,
    };
    let Some((start, path)) = start.zip(arrow_path(style)) else {
        let _ = out.write_str(spec.shape);
        return;
    };

    let [width, height] = spec.viewport.view_box;
    let [reference_x, reference_y] = spec.viewport.reference;
    let [marker_width, marker_height] = spec.viewport.size;
    let (translate, scale) = if start { (width, -width) } else { (0.0, width) };
    let _ = write!(
        out,
        r#" viewBox="0 0 {} {}" refX="{}" refY="{}" markerUnits="userSpaceOnUse" markerWidth="{}" markerHeight="{}" orient="auto"><path d="{path}" transform="matrix({} 0 0 {} {} 0)" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"#,
        number(width),
        number(height),
        number(reference_x),
        number(reference_y),
        number(marker_width),
        number(marker_height),
        number(scale / 10.0),
        number(height / 10.0),
        number(translate),
    );
    if style == MarkerStyle::Open {
        let _ = out.write_str(" fill: none; stroke-linecap: round; stroke-linejoin: round;");
    }
    let _ = out.write_char('"');
}

pub const CLASSIC_POINT_END_VIEW_BOX: [f64; 2] = [10.0, 10.0];

const FLOWCHART_MARKER_SHAPE_SPECS: [FlowchartMarkerShapeSpec; 12] = [
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::PointEnd,
        margin: false,
        marker_class: "marker",
        shape: BaseEdgeMarkerKind::PointEnd.shape_fragment(),
        paint: FlowchartMarkerPaint::StrokeAndFill,
        viewport: FlowchartMarkerViewport {
            view_box: CLASSIC_POINT_END_VIEW_BOX,
            reference: [5.0, 5.0],
            size: [8.0, 8.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::PointStart,
        margin: false,
        marker_class: "marker",
        shape: BaseEdgeMarkerKind::PointStart.shape_fragment(),
        paint: FlowchartMarkerPaint::StrokeAndFill,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [4.5, 5.0],
            size: [8.0, 8.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::PointEnd,
        margin: true,
        marker_class: "marker",
        shape: BaseEdgeMarkerKind::PointEndMargin.shape_fragment(),
        paint: FlowchartMarkerPaint::StrokeAndFill,
        viewport: FlowchartMarkerViewport {
            view_box: [11.5, 14.0],
            reference: [11.5, 7.0],
            size: [10.5, 14.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::PointStart,
        margin: true,
        marker_class: "marker",
        shape: BaseEdgeMarkerKind::PointStartMargin.shape_fragment(),
        paint: FlowchartMarkerPaint::None,
        viewport: FlowchartMarkerViewport {
            view_box: [11.5, 14.0],
            reference: [1.0, 7.0],
            size: [11.5, 14.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CircleEnd,
        margin: false,
        marker_class: "marker",
        shape: BaseEdgeMarkerKind::CircleEnd.shape_fragment(),
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [11.0, 5.0],
            size: [11.0, 11.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CircleStart,
        margin: false,
        marker_class: "marker",
        shape: BaseEdgeMarkerKind::CircleStart.shape_fragment(),
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [-1.0, 5.0],
            size: [11.0, 11.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CircleEnd,
        margin: true,
        marker_class: "marker",
        shape: BaseEdgeMarkerKind::CircleEndMargin.shape_fragment(),
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [12.25, 5.0],
            size: [14.0, 14.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CircleStart,
        margin: true,
        marker_class: "marker",
        shape: BaseEdgeMarkerKind::CircleStartMargin.shape_fragment(),
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [-2.0, 5.0],
            size: [14.0, 14.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CrossEnd,
        margin: false,
        marker_class: "marker cross",
        shape: BaseEdgeMarkerKind::CrossEnd.shape_fragment(),
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [11.0, 11.0],
            reference: [12.0, 5.2],
            size: [11.0, 11.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CrossStart,
        margin: false,
        marker_class: "marker cross",
        shape: BaseEdgeMarkerKind::CrossStart.shape_fragment(),
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [11.0, 11.0],
            reference: [-1.0, 5.2],
            size: [11.0, 11.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CrossEnd,
        margin: true,
        marker_class: "marker cross",
        shape: BaseEdgeMarkerKind::CrossEndMargin.shape_fragment(),
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [15.0, 15.0],
            reference: [17.7, 7.5],
            size: [12.0, 12.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CrossStart,
        margin: true,
        marker_class: "marker cross",
        shape: BaseEdgeMarkerKind::CrossStartMargin.shape_fragment(),
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [15.0, 15.0],
            reference: [-3.5, 7.5],
            size: [12.0, 12.0],
        },
    },
];

const BPMN_MARKER_SHAPE_SPECS: [FlowchartMarkerShapeSpec; 4] = [
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::OpenArrowEnd,
        margin: false,
        marker_class: "marker openArrow",
        shape: r#" viewBox="0 0 12 12" refX="11" refY="6" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 1 1 L 11 6 L 1 11" class="arrowMarkerPath" style="stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; stroke-dasharray: 1, 0; fill: none;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [12.0, 12.0],
            reference: [11.0, 6.0],
            size: [12.0, 12.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::OpenArrowStart,
        margin: false,
        marker_class: "marker openArrow",
        shape: r#" viewBox="0 0 12 12" refX="1" refY="6" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 11 1 L 1 6 L 11 11" class="arrowMarkerPath" style="stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; stroke-dasharray: 1, 0; fill: none;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [12.0, 12.0],
            reference: [1.0, 6.0],
            size: [12.0, 12.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::HollowCircleStart,
        margin: false,
        marker_class: "marker hollowCircle",
        shape: r#" viewBox="0 0 10 10" refX="0" refY="5" markerUnits="userSpaceOnUse" markerWidth="10" markerHeight="10" orient="auto"><circle cx="5" cy="5" r="3.6" class="arrowMarkerPath" style="stroke-width: 1.4; stroke-dasharray: 1, 0; fill: none;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [0.0, 5.0],
            size: [10.0, 10.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::HollowCircleEnd,
        margin: false,
        marker_class: "marker hollowCircle",
        shape: r#" viewBox="0 0 10 10" refX="10" refY="5" markerUnits="userSpaceOnUse" markerWidth="10" markerHeight="10" orient="auto"><circle cx="5" cy="5" r="3.6" class="arrowMarkerPath" style="stroke-width: 1.4; stroke-dasharray: 1, 0; fill: none;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [10.0, 5.0],
            size: [10.0, 10.0],
        },
    },
];

pub fn flowchart_marker_shape_spec(
    base: FlowchartMarkerBase,
    margin: bool,
) -> &'static FlowchartMarkerShapeSpec {
    let margin = margin && base.supports_margin();
    FLOWCHART_MARKER_SHAPE_SPECS
        .iter()
        .chain(BPMN_MARKER_SHAPE_SPECS.iter())
        .find(|spec| spec.base == base && spec.margin == margin)
        .expect("Flowchart marker shape descriptor")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowchartMarkerVariantMode {
    Source,
    AdaptiveInk,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FlowchartMarkerVariantKey {
    pub base: FlowchartMarkerBase,
    pub margin: bool,
    pub color_id: String,
    pub mode: FlowchartMarkerVariantMode,
}

impl FlowchartMarkerVariantKey {
    pub fn color_scan_count(&self) -> usize {
        self.paint().color_scan_count()
    }

    fn paint(&self) -> FlowchartMarkerPaint {
        match self.mode {
            FlowchartMarkerVariantMode::AdaptiveInk
                if matches!(
                    self.base,
                    FlowchartMarkerBase::PointStart
                        | FlowchartMarkerBase::PointEnd
                        | FlowchartMarkerBase::CircleStart
                        | FlowchartMarkerBase::CircleEnd
                ) =>
            {
                FlowchartMarkerPaint::StrokeAndFill
            }
            FlowchartMarkerVariantMode::Source | FlowchartMarkerVariantMode::AdaptiveInk => {
                flowchart_marker_shape_spec(self.base, self.margin).paint
            }
        }
    }
}

pub fn base_marker_count(diagram_type: &str, bpmn_markers: bool) -> usize {
    FLOWCHART_MARKER_SHAPE_SPECS.len()
        + if bpmn_markers {
            BPMN_MARKER_SHAPE_SPECS.len()
        } else {
            0
        }
        + if diagram_type == "agentflow" {
            AGENTFLOW_HIERARCHY_MARKERS.len()
        } else {
            0
        }
}

/// Writes the eager marker set in serialization order for byte counting and emission.
pub fn push_base_markers(
    out: &mut impl fmt::Write,
    diagram_id: &str,
    diagram_type: &str,
    marker_style: MarkerStyle,
    bpmn_markers: bool,
) {
    let id = escape_xml(diagram_id);
    let ty = escape_xml(diagram_type);
    for spec in FLOWCHART_MARKER_SHAPE_SPECS {
        let _ = write!(
            out,
            r#"<marker id="{}_{}-{}"#,
            id.as_str(),
            ty.as_str(),
            spec.base.id_suffix()
        );
        if spec.margin {
            let _ = out.write_str("-margin");
        }
        let _ = write!(out, r#"" class="{} {}""#, spec.marker_class, ty.as_str());
        write_marker_shape(out, &spec, marker_style);
        let _ = out.write_str("/></marker>");
    }
    if diagram_type == "agentflow" {
        // Mermaid eagerly registers these after the ordinary marker set, even without
        // hierarchy edges. Keep them on the same namespace and resource-budget path.
        for (suffix, shape) in AGENTFLOW_HIERARCHY_MARKERS {
            let _ = write!(
                out,
                r#"<marker id="{id}_{ty}-{suffix}" class="marker hierarchy {ty}"{shape}"#,
            );
        }
    }
    if bpmn_markers {
        push_bpmn_markers(out, diagram_id, diagram_type);
    }
}

fn push_bpmn_markers(out: &mut impl fmt::Write, diagram_id: &str, diagram_type: &str) {
    for spec in BPMN_MARKER_SHAPE_SPECS {
        let _ = write!(
            out,
            r#"<marker id="{}_{}-{}" class="{} {}"{}/></marker>"#,
            escape_xml_display(diagram_id),
            escape_xml_display(diagram_type),
            spec.base.id_suffix(),
            spec.marker_class,
            escape_xml_display(diagram_type),
            spec.shape,
        );
    }
}

#[inline]
pub fn write_flowchart_marker_id_xml(
    out: &mut impl fmt::Write,
    diagram_id: &str,
    diagram_type: &str,
    base: FlowchartMarkerBase,
    margin: bool,
    color_id: Option<&str>,
) {
    let _ = write!(out, "{}", escape_xml_display(diagram_id));
    let _ = out.write_char('_');
    let _ = write!(out, "{}", escape_xml_display(diagram_type));
    let _ = out.write_char('-');
    let _ = out.write_str(base.id_suffix());
    if margin {
        let _ = out.write_str("-margin");
    }

    let Some(color_id) = color_id else {
        return;
    };
    let _ = out.write_char('_');
    let _ = out.write_str(color_id);
}

/// Writes one complete source or adaptive-ink marker using the caller's admitted paint.
pub fn push_colored_marker(
    out: &mut impl fmt::Write,
    diagram_id: &str,
    diagram_type: &str,
    key: &FlowchartMarkerVariantKey,
    color: Option<&str>,
    marker_style: MarkerStyle,
) {
    let spec = flowchart_marker_shape_spec(key.base, key.margin);

    let _ = out.write_str(r#"<marker id=""#);
    write_flowchart_marker_id_xml(
        out,
        diagram_id,
        diagram_type,
        key.base,
        key.margin,
        Some(key.color_id.as_str()),
    );
    let _ = write!(
        out,
        r#"" class="{} {}""#,
        spec.marker_class,
        escape_xml_display(diagram_type)
    );
    write_marker_shape(out, spec, marker_style);
    if let Some(color) = color {
        key.paint().push(out, color);
    }
    let _ = out.write_str("/></marker>");
}
