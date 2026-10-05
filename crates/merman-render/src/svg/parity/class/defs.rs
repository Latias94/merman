use super::super::path_bounds::{SvgPathBounds, svg_path_bounds_from_d};
use super::super::*;
use super::ClassSvgRelation;
use std::collections::BTreeSet;

const CLASS_DIAMOND_MARKER_PATH: &str = "M 18,7 L9,13 L1,7 L9,1 Z";
const CLASS_EXTENSION_START_MARKER_PATH: &str = "M 1,7 L18,13 V 1 Z";
const CLASS_EXTENSION_END_MARKER_PATH: &str = "M 1,1 V 13 L18,7 Z";
const CLASS_DEPENDENCY_START_MARKER_PATH: &str = "M 5,7 L9,13 L1,7 L9,1 Z";
const CLASS_DEPENDENCY_END_MARKER_PATH: &str = "M 18,7 L9,13 L14,7 L9,1 Z";

#[derive(Clone, Copy, Debug)]
enum ClassMarkerPaintShape {
    Path(&'static str),
    Circle { cx: f64, cy: f64, radius: f64 },
}

/// Paint geometry for the marker referenced by a Class relation path.
///
/// Mermaid 12 shares marker coordinates and user-space units across layout engines.
#[derive(Clone, Copy, Debug)]
pub(super) struct ClassMarkerPaintSpec {
    ref_x: f64,
    ref_y: f64,
    shape: ClassMarkerPaintShape,
    stroke_outset: f64,
}

impl ClassMarkerPaintSpec {
    pub(super) fn coordinate_scale(self, _relation_stroke_width: f64) -> f64 {
        1.0
    }

    pub(super) fn reference_point(self) -> (f64, f64) {
        (self.ref_x, self.ref_y)
    }

    pub(super) fn local_paint_bounds(self) -> SvgPathBounds {
        let mut bounds = match self.shape {
            ClassMarkerPaintShape::Path(d) => svg_path_bounds_from_d(d)
                .expect("static Class marker paths must have bounded SVG geometry"),
            ClassMarkerPaintShape::Circle { cx, cy, radius } => SvgPathBounds {
                min_x: cx - radius,
                min_y: cy - radius,
                max_x: cx + radius,
                max_y: cy + radius,
            },
        };
        bounds.min_x -= self.stroke_outset;
        bounds.min_y -= self.stroke_outset;
        bounds.max_x += self.stroke_outset;
        bounds.max_y += self.stroke_outset;
        bounds
    }

    pub(super) fn conservative_radius(self, relation_stroke_width: f64) -> f64 {
        let bounds = self.local_paint_bounds();
        let (ref_x, ref_y) = self.reference_point();
        let radius = [
            (bounds.min_x, bounds.min_y),
            (bounds.min_x, bounds.max_y),
            (bounds.max_x, bounds.min_y),
            (bounds.max_x, bounds.max_y),
        ]
        .into_iter()
        .map(|(x, y)| (x - ref_x).hypot(y - ref_y))
        .fold(0.0, f64::max);
        radius * self.coordinate_scale(relation_stroke_width)
    }
}

pub(super) fn class_marker_paint_spec(
    ty: i32,
    is_start: bool,
    margin: bool,
) -> Option<ClassMarkerPaintSpec> {
    let (mut ref_x, ref_y, mut shape) = match (ty, is_start) {
        (0, true) => (
            18.0,
            7.0,
            ClassMarkerPaintShape::Path(CLASS_DIAMOND_MARKER_PATH),
        ),
        (0, false) => (
            1.0,
            7.0,
            ClassMarkerPaintShape::Path(CLASS_DIAMOND_MARKER_PATH),
        ),
        (1, true) => (
            18.0,
            7.0,
            ClassMarkerPaintShape::Path(CLASS_EXTENSION_START_MARKER_PATH),
        ),
        (1, false) => (
            1.0,
            7.0,
            ClassMarkerPaintShape::Path(CLASS_EXTENSION_END_MARKER_PATH),
        ),
        (2, true) => (
            18.0,
            7.0,
            ClassMarkerPaintShape::Path(CLASS_DIAMOND_MARKER_PATH),
        ),
        (2, false) => (
            1.0,
            7.0,
            ClassMarkerPaintShape::Path(CLASS_DIAMOND_MARKER_PATH),
        ),
        (3, true) => (
            6.0,
            7.0,
            ClassMarkerPaintShape::Path(CLASS_DEPENDENCY_START_MARKER_PATH),
        ),
        (3, false) => (
            13.0,
            7.0,
            ClassMarkerPaintShape::Path(CLASS_DEPENDENCY_END_MARKER_PATH),
        ),
        (4, true) => (
            13.0,
            7.0,
            ClassMarkerPaintShape::Circle {
                cx: 7.0,
                cy: 7.0,
                radius: 6.0,
            },
        ),
        (4, false) => (
            1.0,
            7.0,
            ClassMarkerPaintShape::Circle {
                cx: 7.0,
                cy: 7.0,
                radius: 6.0,
            },
        ),
        _ => return None,
    };

    // Bound the full miter join for path strokes, and half the width for circles.
    let mut stroke_outset = if ty == 4 { 0.5 } else { 4.0 };
    if margin {
        match ty {
            0 => {
                ref_x = if is_start { 15.0 } else { 1.0 };
                stroke_outset = 8.0;
            }
            1 => {
                ref_x = if is_start { 18.0 } else { 9.0 };
                shape = ClassMarkerPaintShape::Path(if is_start {
                    "M10,7 L18,13 L18,1 Z"
                } else {
                    "M10,1 L10,13 L18,7 Z"
                });
                stroke_outset = 8.0;
            }
            2 => {
                ref_x = if is_start { 15.0 } else { 3.5 };
                stroke_outset = 0.0;
            }
            3 => {
                ref_x = if is_start { 4.0 } else { 16.0 };
                stroke_outset = 0.0;
            }
            4 => stroke_outset = 1.0,
            _ => unreachable!("validated Class marker type"),
        }
    }
    Some(ClassMarkerPaintSpec {
        ref_x,
        ref_y,
        shape,
        stroke_outset,
    })
}

pub(super) fn class_marker_name(ty: i32, is_start: bool) -> Option<&'static str> {
    // Mermaid class diagram relationType constants.
    // -1 = none, 0 = aggregation, 1 = extension, 2 = composition, 3 = dependency, 4 = lollipop
    match ty {
        0 => Some(if is_start {
            "aggregationStart"
        } else {
            "aggregationEnd"
        }),
        1 => Some(if is_start {
            "extensionStart"
        } else {
            "extensionEnd"
        }),
        2 => Some(if is_start {
            "compositionStart"
        } else {
            "compositionEnd"
        }),
        3 => Some(if is_start {
            "dependencyStart"
        } else {
            "dependencyEnd"
        }),
        4 => Some(if is_start {
            "lollipopStart"
        } else {
            "lollipopEnd"
        }),
        _ => None,
    }
}

const CLASS_RELATION_MARKER_ORDER: [&str; 20] = [
    "aggregationStart",
    "aggregationEnd",
    "aggregationStart-margin",
    "aggregationEnd-margin",
    "extensionStart",
    "extensionEnd",
    "extensionStart-margin",
    "extensionEnd-margin",
    "compositionStart",
    "compositionEnd",
    "compositionStart-margin",
    "compositionEnd-margin",
    "dependencyStart",
    "dependencyEnd",
    "dependencyStart-margin",
    "dependencyEnd-margin",
    "lollipopStart",
    "lollipopEnd",
    "lollipopStart-margin",
    "lollipopEnd-margin",
];

fn class_marker_fill_follows_stroke(marker_name: &str) -> bool {
    marker_name.starts_with("composition") || marker_name.starts_with("dependency")
}

fn class_marker_has_transparent_terminal_fill(marker_name: &str) -> bool {
    matches!(
        marker_name.strip_suffix("-margin").unwrap_or(marker_name),
        "aggregationStart" | "aggregationEnd" | "extensionStart" | "extensionEnd"
    )
}

pub(super) fn class_marker_terminal_expectations(
    relations: &[ClassSvgRelation],
    include_margin_markers: bool,
) -> Vec<crate::class::ClassMarkerTerminalExpectation> {
    let referenced = relations
        .iter()
        .flat_map(|relation| {
            [
                class_marker_name(relation.relation.type1, true),
                class_marker_name(relation.relation.type2, false),
            ]
            .into_iter()
            .flatten()
        })
        .collect::<BTreeSet<_>>();
    CLASS_RELATION_MARKER_ORDER
        .into_iter()
        .filter(|name| {
            let ordinary = name.strip_suffix("-margin");
            (ordinary.is_none() || include_margin_markers)
                && referenced.contains(ordinary.unwrap_or(name))
        })
        .map(|name| {
            crate::class::ClassMarkerTerminalExpectation::new(
                name,
                class_marker_fill_follows_stroke(name),
            )
        })
        .collect()
}

pub(super) fn class_markers<I: SvgDiagramIdValue>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    diagram_marker_class: &str,
    include_margin_markers: bool,
    relation_theme: &crate::class::ClassRelationThemePlan,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
) -> Result<()> {
    // Match Mermaid unified output: multiple <defs> wrappers, one marker each.
    struct MarkerContext<'a, O: SvgOutput, I: SvgDiagramIdValue> {
        out: &'a mut O,
        diagram_id: I,
        diagram_marker_class: &'a str,
        relation_theme: &'a crate::class::ClassRelationThemePlan,
        theme_receipt: &'a mut crate::class::ClassRelationThemeReceipt,
    }

    enum MarkerShape<'a> {
        Path(&'a str),
        StyledPath {
            d: &'a str,
            view_box: Option<&'a str>,
            stroke_width: &'a str,
        },
        Polygon(&'a str),
        Circle {
            stroke: Option<&'a str>,
            stroke_width: Option<&'a str>,
        },
    }

    struct MarkerSpec<'a> {
        name: &'static str,
        kind: &'a str,
        ref_x: &'a str,
        ref_y: &'a str,
        marker_w: &'a str,
        marker_h: &'a str,
        marker_units: Option<&'a str>,
        view_box: Option<&'a str>,
        wrap_defs: bool,
        shape: MarkerShape<'a>,
    }

    fn marker<O: SvgOutput, I: SvgDiagramIdValue>(
        ctx: &mut MarkerContext<'_, O, I>,
        spec: MarkerSpec<'_>,
    ) -> Result<()> {
        let typed_stroke = if ctx.theme_receipt.themes_marker(spec.name) {
            ctx.relation_theme.typed_stroke()
        } else {
            None
        };
        let fill_follows_stroke = class_marker_fill_follows_stroke(spec.name);
        let terminal_style = typed_stroke.map(|(_, css)| {
            if fill_follows_stroke {
                format!("stroke:{css} !important;fill:{css} !important")
            } else if class_marker_has_transparent_terminal_fill(spec.name) {
                format!("stroke:{css} !important;fill:transparent !important")
            } else {
                format!("stroke:{css} !important")
            }
        });
        if spec.wrap_defs {
            ctx.out.push_str("<defs>");
            ctx.out.checkpoint()?;
        }
        match spec.shape {
            MarkerShape::Path(d) | MarkerShape::StyledPath { d, .. } => {
                let _ = write!(
                    ctx.out,
                    r#"<marker id="{}_{}-{}" class="marker {} {}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto""#,
                    ctx.diagram_id,
                    escape_xml_display(ctx.diagram_marker_class),
                    escape_xml_display(spec.name),
                    escape_xml_display(spec.kind),
                    escape_xml_display(ctx.diagram_marker_class),
                    spec.ref_x,
                    spec.ref_y,
                    spec.marker_w,
                    spec.marker_h,
                );
                if let Some(marker_units) = spec.marker_units {
                    let _ = write!(ctx.out, r#" markerUnits="{}""#, marker_units);
                }
                if let Some(view_box) = spec.view_box {
                    let _ = write!(ctx.out, r#" viewBox="{}""#, view_box);
                }
                let _ = write!(ctx.out, r#"><path d="{}""#, escape_xml_display(d));
                if let MarkerShape::StyledPath {
                    view_box,
                    stroke_width,
                    ..
                } = spec.shape
                {
                    if let Some(view_box) = view_box {
                        let _ = write!(ctx.out, r#" viewBox="{}""#, view_box);
                    }
                    let _ = write!(ctx.out, r#" style="stroke-width: {};"#, stroke_width);
                    if let Some(style) = terminal_style.as_deref() {
                        let _ = write!(ctx.out, "{}", escape_xml_display(style));
                    }
                    ctx.out.push('"');
                } else if let Some(style) = terminal_style.as_deref() {
                    let _ = write!(ctx.out, r#" style="{}""#, escape_xml_display(style));
                }
                ctx.out.push_str("/></marker>");
            }
            MarkerShape::Polygon(points) => {
                let _ = write!(
                    ctx.out,
                    r#"<marker id="{}_{}-{}" class="marker {} {}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto""#,
                    ctx.diagram_id,
                    escape_xml_display(ctx.diagram_marker_class),
                    escape_xml_display(spec.name),
                    escape_xml_display(spec.kind),
                    escape_xml_display(ctx.diagram_marker_class),
                    spec.ref_x,
                    spec.ref_y,
                    spec.marker_w,
                    spec.marker_h,
                );
                if let Some(marker_units) = spec.marker_units {
                    let _ = write!(ctx.out, r#" markerUnits="{}""#, marker_units);
                }
                if let Some(view_box) = spec.view_box {
                    let _ = write!(ctx.out, r#" viewBox="{}""#, view_box);
                }
                let _ = write!(
                    ctx.out,
                    r#"><polygon points="{}""#,
                    escape_xml_display(points)
                );
                ctx.out
                    .push_str(r#" style="stroke-width: 2; stroke-dasharray: 0;"#);
                if let Some(style) = terminal_style.as_deref() {
                    let _ = write!(ctx.out, "{}", escape_xml_display(style));
                }
                ctx.out.push('"');
                ctx.out.push_str("/></marker>");
            }
            MarkerShape::Circle {
                stroke,
                stroke_width,
            } => {
                let _ = write!(
                    ctx.out,
                    r#"<marker id="{}_{}-{}" class="marker {} {}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto""#,
                    ctx.diagram_id,
                    escape_xml_display(ctx.diagram_marker_class),
                    escape_xml_display(spec.name),
                    escape_xml_display(spec.kind),
                    escape_xml_display(ctx.diagram_marker_class),
                    spec.ref_x,
                    spec.ref_y,
                    spec.marker_w,
                    spec.marker_h,
                );
                if let Some(marker_units) = spec.marker_units {
                    let _ = write!(ctx.out, r#" markerUnits="{}""#, marker_units);
                }
                if let Some(view_box) = spec.view_box {
                    let _ = write!(ctx.out, r#" viewBox="{}""#, view_box);
                }
                ctx.out
                    .push_str(r#"><circle fill="transparent" cx="7" cy="7" r="6""#);
                if let Some(stroke) = stroke {
                    let _ = write!(ctx.out, r#" stroke="{}""#, escape_xml_display(stroke));
                }
                if let Some(stroke_width) = stroke_width {
                    let _ = write!(ctx.out, r#" stroke-width="{}""#, stroke_width);
                }
                if let Some(style) = terminal_style.as_deref() {
                    let _ = write!(ctx.out, r#" style="{}""#, escape_xml_display(style));
                }
                ctx.out.push_str("/></marker>");
            }
        }
        if spec.wrap_defs {
            ctx.out.push_str("</defs>");
        }
        ctx.out.checkpoint()?;
        if typed_stroke.is_some() {
            ctx.theme_receipt.record_marker(
                spec.name,
                fill_follows_stroke,
                typed_stroke,
                terminal_style.as_deref(),
            );
        }
        Ok(())
    }

    let mut ctx = MarkerContext {
        out,
        diagram_id,
        diagram_marker_class,
        relation_theme,
        theme_receipt,
    };

    marker(
        &mut ctx,
        MarkerSpec {
            name: "aggregationStart",
            kind: "aggregation",
            ref_x: "18",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Path(CLASS_DIAMOND_MARKER_PATH),
        },
    )?;
    marker(
        &mut ctx,
        MarkerSpec {
            name: "aggregationEnd",
            kind: "aggregation",
            ref_x: "1",
            ref_y: "7",
            marker_w: "20",
            marker_h: "28",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Path(CLASS_DIAMOND_MARKER_PATH),
        },
    )?;
    if include_margin_markers {
        marker(
            &mut ctx,
            MarkerSpec {
                name: "aggregationStart-margin",
                kind: "aggregation",
                ref_x: "15",
                ref_y: "7",
                marker_w: "190",
                marker_h: "240",
                marker_units: Some("userSpaceOnUse"),
                view_box: None,
                wrap_defs: true,
                shape: MarkerShape::StyledPath {
                    d: "M 18,7 L9,13 L1,7 L9,1 Z",
                    view_box: None,
                    stroke_width: "2",
                },
            },
        )?;
        marker(
            &mut ctx,
            MarkerSpec {
                name: "aggregationEnd-margin",
                kind: "aggregation",
                ref_x: "1",
                ref_y: "7",
                marker_w: "20",
                marker_h: "28",
                marker_units: Some("userSpaceOnUse"),
                view_box: None,
                wrap_defs: true,
                shape: MarkerShape::StyledPath {
                    d: "M 18,7 L9,13 L1,7 L9,1 Z",
                    view_box: None,
                    stroke_width: "2",
                },
            },
        )?;
    }

    marker(
        &mut ctx,
        MarkerSpec {
            name: "extensionStart",
            kind: "extension",
            ref_x: "18",
            ref_y: "7",
            marker_w: "20",
            marker_h: "28",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Path(CLASS_EXTENSION_START_MARKER_PATH),
        },
    )?;
    marker(
        &mut ctx,
        MarkerSpec {
            name: "extensionEnd",
            kind: "extension",
            ref_x: "1",
            ref_y: "7",
            marker_w: "20",
            marker_h: "28",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Path(CLASS_EXTENSION_END_MARKER_PATH),
        },
    )?;
    if include_margin_markers {
        marker(
            &mut ctx,
            MarkerSpec {
                name: "extensionStart-margin",
                kind: "extension",
                ref_x: "18",
                ref_y: "7",
                marker_w: "20",
                marker_h: "28",
                marker_units: Some("userSpaceOnUse"),
                view_box: Some("0 0 20 14"),
                wrap_defs: false,
                shape: MarkerShape::Polygon("10,7 18,13 18,1"),
            },
        )?;
        marker(
            &mut ctx,
            MarkerSpec {
                name: "extensionEnd-margin",
                kind: "extension",
                ref_x: "9",
                ref_y: "7",
                marker_w: "20",
                marker_h: "28",
                marker_units: Some("userSpaceOnUse"),
                view_box: Some("0 0 20 14"),
                wrap_defs: true,
                shape: MarkerShape::Polygon("10,1 10,13 18,7"),
            },
        )?;
    }

    marker(
        &mut ctx,
        MarkerSpec {
            name: "compositionStart",
            kind: "composition",
            ref_x: "18",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Path(CLASS_DIAMOND_MARKER_PATH),
        },
    )?;
    marker(
        &mut ctx,
        MarkerSpec {
            name: "compositionEnd",
            kind: "composition",
            ref_x: "1",
            ref_y: "7",
            marker_w: "20",
            marker_h: "28",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Path(CLASS_DIAMOND_MARKER_PATH),
        },
    )?;
    if include_margin_markers {
        marker(
            &mut ctx,
            MarkerSpec {
                name: "compositionStart-margin",
                kind: "composition",
                ref_x: "15",
                ref_y: "7",
                marker_w: "190",
                marker_h: "240",
                marker_units: Some("userSpaceOnUse"),
                view_box: None,
                wrap_defs: true,
                shape: MarkerShape::StyledPath {
                    d: "M 18,7 L9,13 L1,7 L9,1 Z",
                    view_box: Some("0 0 15 15"),
                    stroke_width: "0",
                },
            },
        )?;
        marker(
            &mut ctx,
            MarkerSpec {
                name: "compositionEnd-margin",
                kind: "composition",
                ref_x: "3.5",
                ref_y: "7",
                marker_w: "20",
                marker_h: "28",
                marker_units: Some("userSpaceOnUse"),
                view_box: None,
                wrap_defs: true,
                shape: MarkerShape::StyledPath {
                    d: "M 18,7 L9,13 L1,7 L9,1 Z",
                    view_box: None,
                    stroke_width: "0",
                },
            },
        )?;
    }

    marker(
        &mut ctx,
        MarkerSpec {
            name: "dependencyStart",
            kind: "dependency",
            ref_x: "6",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Path(CLASS_DEPENDENCY_START_MARKER_PATH),
        },
    )?;
    marker(
        &mut ctx,
        MarkerSpec {
            name: "dependencyEnd",
            kind: "dependency",
            ref_x: "13",
            ref_y: "7",
            marker_w: "20",
            marker_h: "28",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Path(CLASS_DEPENDENCY_END_MARKER_PATH),
        },
    )?;
    if include_margin_markers {
        marker(
            &mut ctx,
            MarkerSpec {
                name: "dependencyStart-margin",
                kind: "dependency",
                ref_x: "4",
                ref_y: "7",
                marker_w: "190",
                marker_h: "240",
                marker_units: Some("userSpaceOnUse"),
                view_box: None,
                wrap_defs: true,
                shape: MarkerShape::StyledPath {
                    d: "M 5,7 L9,13 L1,7 L9,1 Z",
                    view_box: None,
                    stroke_width: "0",
                },
            },
        )?;
        marker(
            &mut ctx,
            MarkerSpec {
                name: "dependencyEnd-margin",
                kind: "dependency",
                ref_x: "16",
                ref_y: "7",
                marker_w: "20",
                marker_h: "28",
                marker_units: Some("userSpaceOnUse"),
                view_box: None,
                wrap_defs: true,
                shape: MarkerShape::StyledPath {
                    d: "M 18,7 L9,13 L14,7 L9,1 Z",
                    view_box: None,
                    stroke_width: "0",
                },
            },
        )?;
    }

    marker(
        &mut ctx,
        MarkerSpec {
            name: "lollipopStart",
            kind: "lollipop",
            ref_x: "13",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Circle {
                stroke: (!include_margin_markers).then_some("black"),
                stroke_width: None,
            },
        },
    )?;
    marker(
        &mut ctx,
        MarkerSpec {
            name: "lollipopEnd",
            kind: "lollipop",
            ref_x: "1",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            marker_units: Some("userSpaceOnUse"),
            view_box: None,
            wrap_defs: true,
            shape: MarkerShape::Circle {
                stroke: (!include_margin_markers).then_some("black"),
                stroke_width: None,
            },
        },
    )?;
    if include_margin_markers {
        marker(
            &mut ctx,
            MarkerSpec {
                name: "lollipopStart-margin",
                kind: "lollipop",
                ref_x: "13",
                ref_y: "7",
                marker_w: "190",
                marker_h: "240",
                marker_units: Some("userSpaceOnUse"),
                view_box: None,
                wrap_defs: true,
                shape: MarkerShape::Circle {
                    stroke: None,
                    stroke_width: Some("2"),
                },
            },
        )?;
        marker(
            &mut ctx,
            MarkerSpec {
                name: "lollipopEnd-margin",
                kind: "lollipop",
                ref_x: "1",
                ref_y: "7",
                marker_w: "190",
                marker_h: "240",
                marker_units: Some("userSpaceOnUse"),
                view_box: None,
                wrap_defs: true,
                shape: MarkerShape::Circle {
                    stroke: None,
                    stroke_width: Some("2"),
                },
            },
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::ops::Range;

    #[test]
    fn relation_marker_paint_specs_match_the_emitted_coordinate_contract() {
        let relation_theme = crate::class::ClassRelationThemePlan::default();
        {
            let mut svg = String::new();
            let mut receipt = relation_theme.begin_terminal_receipt(Vec::new(), Vec::new(), false);
            class_markers(
                &mut svg,
                "diagram",
                "class",
                true,
                &relation_theme,
                &mut receipt,
            )
            .expect("render Class markers");

            let mut previous_marker = None;
            for name in CLASS_RELATION_MARKER_ORDER {
                let offset = svg.find(&format!(r#"id="diagram_class-{name}""#)).unwrap();
                assert!(previous_marker.is_none_or(|previous| previous < offset));
                previous_marker = Some(offset);
            }

            for (ty, is_start, margin) in (0..=4).flat_map(|ty| {
                [
                    (ty, true, false),
                    (ty, false, false),
                    (ty, true, true),
                    (ty, false, true),
                ]
            }) {
                let ordinary = class_marker_name(ty, is_start).expect("Class marker name");
                let marker_name = format!("{ordinary}{}", if margin { "-margin" } else { "" });
                let marker =
                    class_marker_paint_spec(ty, is_start, margin).expect("Class marker paint spec");
                let marker_id = format!(r#"id="diagram_class-{marker_name}""#);
                let marker_start = svg.find(&marker_id).expect("emitted Class marker");
                let marker_opening = &svg[marker_start..];
                let marker_opening = &marker_opening[..marker_opening
                    .find('>')
                    .expect("complete Class marker opening tag")];
                let (ref_x, ref_y) = marker.reference_point();

                assert!(marker_opening.contains(&format!(r#"refX="{ref_x}""#)));
                assert!(marker_opening.contains(&format!(r#"refY="{ref_y}""#)));
                assert!(
                    marker_opening.contains(r#"markerUnits="userSpaceOnUse""#),
                    "marker={marker_name} opening={marker_opening}"
                );
                assert!(
                    marker.local_paint_bounds().min_x.is_finite(),
                    "marker={marker_name} must have finite paint geometry"
                );
            }
        }
    }

    #[derive(Default)]
    struct RejectAfterFirstWrite {
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterFirstWrite {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts == 1 {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterFirstWrite {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl SvgOutput for RejectAfterFirstWrite {
        fn push_str(&mut self, value: &str) {
            let _ = self.record_write(value);
        }

        fn push(&mut self, value: char) {
            let mut encoded = [0u8; 4];
            let _ = self.record_write(value.encode_utf8(&mut encoded));
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> crate::Result<()> {
            if self.rejected {
                Err(crate::Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn class_markers_stop_after_the_first_svg_sink_failure() {
        let mut out = RejectAfterFirstWrite::default();
        let relation_theme = crate::class::ClassRelationThemePlan::default();
        let mut receipt = relation_theme.begin_terminal_receipt(Vec::new(), Vec::new(), false);

        let error = class_markers(
            &mut out,
            "class-sink-failure",
            "classDiagram",
            true,
            &relation_theme,
            &mut receipt,
        )
        .expect_err("the rejecting sink must stop Class marker rendering");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Class marker rendering must stop at the first failed sink checkpoint"
        );
    }
}
