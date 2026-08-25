use super::super::path_bounds::{SvgPathBounds, svg_path_bounds_from_d};
use super::super::*;
use super::ClassSvgRelation;
use std::collections::BTreeSet;

const CLASS_DIAMOND_MARKER_PATH: &str = "M 18,7 L9,13 L1,7 L9,1 Z";
const CLASS_EXTENSION_START_MARKER_PATH: &str = "M 1,7 L18,13 V 1 Z";
const CLASS_EXTENSION_END_MARKER_PATH: &str = "M 1,1 V 13 L18,7 Z";
const CLASS_DEPENDENCY_START_MARKER_PATH: &str = "M 5,7 L9,13 L1,7 L9,1 Z";
const CLASS_DEPENDENCY_END_MARKER_PATH: &str = "M 18,7 L9,13 L14,7 L9,1 Z";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClassMarkerCoordinateUnits {
    StrokeWidth,
    UserSpaceOnUse,
}

#[derive(Clone, Copy, Debug)]
enum ClassMarkerPaintShape {
    Path(&'static str),
    Circle { cx: f64, cy: f64, radius: f64 },
}

/// Paint geometry for the ordinary marker referenced by a Class relation path.
///
/// The margin marker variants are emitted for Mermaid DOM parity but relation paths currently
/// reference the ordinary marker ids. Most ordinary markers use SVG's default
/// `markerUnits="strokeWidth"`; `extensionStart` is the upstream exception and uses
/// `userSpaceOnUse` when the complete marker set is emitted.
#[derive(Clone, Copy, Debug)]
pub(super) struct ClassMarkerPaintSpec {
    ref_x: f64,
    ref_y: f64,
    units: ClassMarkerCoordinateUnits,
    shape: ClassMarkerPaintShape,
}

impl ClassMarkerPaintSpec {
    pub(super) fn coordinate_scale(self, relation_stroke_width: f64) -> f64 {
        match self.units {
            ClassMarkerCoordinateUnits::StrokeWidth => relation_stroke_width.max(0.0),
            ClassMarkerCoordinateUnits::UserSpaceOnUse => 1.0,
        }
    }

    pub(super) fn reference_point(self) -> (f64, f64) {
        (self.ref_x, self.ref_y)
    }

    pub(super) fn local_paint_bounds(self) -> SvgPathBounds {
        let (mut bounds, stroke_outset) = match self.shape {
            ClassMarkerPaintShape::Path(d) => {
                // Marker CSS uses stroke-width 1 and the SVG defaults retain miter joins with a
                // miter limit of 4. One full stroke times that limit conservatively covers the
                // complete join extent for every path marker corner before viewport clipping.
                (
                    svg_path_bounds_from_d(d)
                        .expect("static Class marker paths must have bounded SVG geometry"),
                    4.0,
                )
            }
            ClassMarkerPaintShape::Circle { cx, cy, radius } => (
                SvgPathBounds {
                    min_x: cx - radius,
                    min_y: cy - radius,
                    max_x: cx + radius,
                    max_y: cy + radius,
                },
                0.5,
            ),
        };
        bounds.min_x -= stroke_outset;
        bounds.min_y -= stroke_outset;
        bounds.max_x += stroke_outset;
        bounds.max_y += stroke_outset;
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

pub(super) fn class_marker_paint_spec(ty: i32, is_start: bool) -> Option<ClassMarkerPaintSpec> {
    use ClassMarkerCoordinateUnits::{StrokeWidth, UserSpaceOnUse};

    let (ref_x, ref_y, units, shape) = match (ty, is_start) {
        (0, true) => (
            18.0,
            7.0,
            StrokeWidth,
            ClassMarkerPaintShape::Path(CLASS_DIAMOND_MARKER_PATH),
        ),
        (0, false) => (
            1.0,
            7.0,
            StrokeWidth,
            ClassMarkerPaintShape::Path(CLASS_DIAMOND_MARKER_PATH),
        ),
        (1, true) => (
            18.0,
            7.0,
            UserSpaceOnUse,
            ClassMarkerPaintShape::Path(CLASS_EXTENSION_START_MARKER_PATH),
        ),
        (1, false) => (
            1.0,
            7.0,
            StrokeWidth,
            ClassMarkerPaintShape::Path(CLASS_EXTENSION_END_MARKER_PATH),
        ),
        (2, true) => (
            18.0,
            7.0,
            StrokeWidth,
            ClassMarkerPaintShape::Path(CLASS_DIAMOND_MARKER_PATH),
        ),
        (2, false) => (
            1.0,
            7.0,
            StrokeWidth,
            ClassMarkerPaintShape::Path(CLASS_DIAMOND_MARKER_PATH),
        ),
        (3, true) => (
            6.0,
            7.0,
            StrokeWidth,
            ClassMarkerPaintShape::Path(CLASS_DEPENDENCY_START_MARKER_PATH),
        ),
        (3, false) => (
            13.0,
            7.0,
            StrokeWidth,
            ClassMarkerPaintShape::Path(CLASS_DEPENDENCY_END_MARKER_PATH),
        ),
        (4, true) => (
            13.0,
            7.0,
            StrokeWidth,
            ClassMarkerPaintShape::Circle {
                cx: 7.0,
                cy: 7.0,
                radius: 6.0,
            },
        ),
        (4, false) => (
            1.0,
            7.0,
            StrokeWidth,
            ClassMarkerPaintShape::Circle {
                cx: 7.0,
                cy: 7.0,
                radius: 6.0,
            },
        ),
        _ => return None,
    };

    Some(ClassMarkerPaintSpec {
        ref_x,
        ref_y,
        units,
        shape,
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

const CLASS_RELATION_MARKER_ORDER: [&str; 10] = [
    "aggregationStart",
    "aggregationEnd",
    "extensionStart",
    "extensionEnd",
    "compositionStart",
    "compositionEnd",
    "dependencyStart",
    "dependencyEnd",
    "lollipopStart",
    "lollipopEnd",
];

fn class_marker_fill_follows_stroke(marker_name: &str) -> bool {
    marker_name.starts_with("composition") || marker_name.starts_with("dependency")
}

fn class_marker_has_transparent_terminal_fill(marker_name: &str) -> bool {
    matches!(
        marker_name,
        "aggregationStart" | "aggregationEnd" | "extensionStart" | "extensionEnd"
    )
}

pub(super) fn class_marker_terminal_expectations(
    relations: &[ClassSvgRelation],
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
        .filter(|name| referenced.contains(name))
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
        PathWithViewBox(&'a str, &'a str),
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
            MarkerShape::Path(d) | MarkerShape::PathWithViewBox(d, _) => {
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
                if let MarkerShape::PathWithViewBox(_, path_view_box) = spec.shape {
                    let _ = write!(
                        ctx.out,
                        r#"><path d="{}" viewBox="{}""#,
                        escape_xml_display(d),
                        path_view_box
                    );
                } else {
                    let _ = write!(ctx.out, r#"><path d="{}""#, escape_xml_display(d));
                }
                if let Some(style) = terminal_style.as_deref() {
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
                if let Some(style) = terminal_style.as_deref() {
                    let _ = write!(ctx.out, r#" style="{}""#, escape_xml_display(style));
                }
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
            marker_units: None,
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
            marker_units: None,
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
                shape: MarkerShape::Path(CLASS_DIAMOND_MARKER_PATH),
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
                shape: MarkerShape::Path(CLASS_DIAMOND_MARKER_PATH),
            },
        )?;
    }

    let (extension_start_marker_w, extension_start_marker_h, extension_start_marker_units) =
        if include_margin_markers {
            ("20", "28", Some("userSpaceOnUse"))
        } else {
            ("190", "240", None)
        };
    marker(
        &mut ctx,
        MarkerSpec {
            name: "extensionStart",
            kind: "extension",
            ref_x: "18",
            ref_y: "7",
            marker_w: extension_start_marker_w,
            marker_h: extension_start_marker_h,
            marker_units: extension_start_marker_units,
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
            marker_units: None,
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
            marker_units: None,
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
            marker_units: None,
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
                shape: MarkerShape::PathWithViewBox(CLASS_DIAMOND_MARKER_PATH, "0 0 15 15"),
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
                shape: MarkerShape::Path(CLASS_DIAMOND_MARKER_PATH),
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
            marker_units: None,
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
            marker_units: None,
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
                shape: MarkerShape::Path(CLASS_DEPENDENCY_START_MARKER_PATH),
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
                shape: MarkerShape::Path(CLASS_DEPENDENCY_END_MARKER_PATH),
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
            marker_units: None,
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
            marker_units: None,
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

pub(super) fn push_class_shadow_defs<I: SvgDiagramIdValue>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    effective_config_value: &serde_json::Value,
) -> Result<()> {
    let flood_color = effective_config_value
        .get("theme")
        .and_then(|v| v.as_str())
        .filter(|theme| theme.contains("dark"))
        .map(|_| "#FFFFFF")
        .unwrap_or("#000000");
    let _ = write!(
        out,
        r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}-drop-shadow-small" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        diagram_id, flood_color, diagram_id, flood_color
    );
    out.checkpoint()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::ops::Range;

    #[test]
    fn relation_marker_paint_specs_match_the_emitted_coordinate_contract() {
        let mut svg = String::new();
        let relation_theme = crate::class::ClassRelationThemePlan::default();
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

        for (ty, is_start) in (0..=4).flat_map(|ty| [(ty, true), (ty, false)]) {
            let marker_name = class_marker_name(ty, is_start).expect("Class marker name");
            let marker = class_marker_paint_spec(ty, is_start).expect("Class marker paint spec");
            let marker_id = format!(r#"id="diagram_class-{marker_name}""#);
            let marker_start = svg.find(&marker_id).expect("emitted Class marker");
            let marker_opening = &svg[marker_start..];
            let marker_opening = &marker_opening[..marker_opening
                .find('>')
                .expect("complete Class marker opening tag")];
            let (ref_x, ref_y) = marker.reference_point();

            assert!(marker_opening.contains(&format!(r#"refX="{ref_x}""#)));
            assert!(marker_opening.contains(&format!(r#"refY="{ref_y}""#)));
            assert_eq!(
                marker_opening.contains(r#"markerUnits="userSpaceOnUse""#),
                marker.units == ClassMarkerCoordinateUnits::UserSpaceOnUse,
                "marker={marker_name} opening={marker_opening}"
            );
            assert!(
                marker.local_paint_bounds().min_x.is_finite(),
                "marker={marker_name} must have finite paint geometry"
            );
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

pub(super) fn push_class_gradient<I: SvgDiagramIdValue>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    effective_config_value: &serde_json::Value,
) -> Result<()> {
    if !config_bool(effective_config_value, &["themeVariables", "useGradient"]).unwrap_or(false) {
        return Ok(());
    }

    let gradient_start =
        config_string(effective_config_value, &["themeVariables", "gradientStart"])
            .or_else(|| {
                config_string(
                    effective_config_value,
                    &["themeVariables", "primaryBorderColor"],
                )
            })
            .unwrap_or_else(|| "#9370DB".to_string());
    let gradient_stop = config_string(effective_config_value, &["themeVariables", "gradientStop"])
        .or_else(|| {
            config_string(
                effective_config_value,
                &["themeVariables", "secondaryBorderColor"],
            )
        })
        .unwrap_or_else(|| gradient_start.clone());

    let gradient_start = escape_xml(&gradient_start);
    let gradient_stop = escape_xml(&gradient_stop);
    let _ = write!(
        out,
        r#"<linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
        diagram_id,
        gradient_start.as_str(),
        gradient_stop.as_str()
    );
    out.checkpoint()
}
