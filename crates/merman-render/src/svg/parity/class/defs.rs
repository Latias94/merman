use super::super::*;

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

pub(super) fn class_markers(out: &mut String, diagram_id: &str, diagram_marker_class: &str) {
    // Match Mermaid unified output: multiple <defs> wrappers, one marker each.
    struct MarkerContext<'a> {
        out: &'a mut String,
        diagram_id: &'a str,
        diagram_marker_class: &'a str,
    }

    enum MarkerShape<'a> {
        Path(&'a str),
        Circle,
    }

    struct MarkerSpec<'a> {
        name: &'a str,
        kind: &'a str,
        ref_x: &'a str,
        ref_y: &'a str,
        marker_w: &'a str,
        marker_h: &'a str,
        shape: MarkerShape<'a>,
    }

    fn marker(ctx: &mut MarkerContext<'_>, spec: MarkerSpec<'_>) {
        match spec.shape {
            MarkerShape::Path(d) => {
                let _ = write!(
                    ctx.out,
                    r#"<defs><marker id="{}_{}-{}" class="marker {} {}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto"><path d="{}"/></marker></defs>"#,
                    escape_xml_display(ctx.diagram_id),
                    escape_xml_display(ctx.diagram_marker_class),
                    escape_xml_display(spec.name),
                    escape_xml_display(spec.kind),
                    escape_xml_display(ctx.diagram_marker_class),
                    spec.ref_x,
                    spec.ref_y,
                    spec.marker_w,
                    spec.marker_h,
                    escape_xml_display(d)
                );
            }
            MarkerShape::Circle => {
                let _ = write!(
                    ctx.out,
                    r#"<defs><marker id="{}_{}-{}" class="marker {} {}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto"><circle stroke="black" fill="transparent" cx="7" cy="7" r="6"/></marker></defs>"#,
                    escape_xml_display(ctx.diagram_id),
                    escape_xml_display(ctx.diagram_marker_class),
                    escape_xml_display(spec.name),
                    escape_xml_display(spec.kind),
                    escape_xml_display(ctx.diagram_marker_class),
                    spec.ref_x,
                    spec.ref_y,
                    spec.marker_w,
                    spec.marker_h
                );
            }
        }
    }

    let mut ctx = MarkerContext {
        out,
        diagram_id,
        diagram_marker_class,
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
            shape: MarkerShape::Path("M 18,7 L9,13 L1,7 L9,1 Z"),
        },
    );
    marker(
        &mut ctx,
        MarkerSpec {
            name: "aggregationEnd",
            kind: "aggregation",
            ref_x: "1",
            ref_y: "7",
            marker_w: "20",
            marker_h: "28",
            shape: MarkerShape::Path("M 18,7 L9,13 L1,7 L9,1 Z"),
        },
    );

    marker(
        &mut ctx,
        MarkerSpec {
            name: "extensionStart",
            kind: "extension",
            ref_x: "18",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            shape: MarkerShape::Path("M 1,7 L18,13 V 1 Z"),
        },
    );
    marker(
        &mut ctx,
        MarkerSpec {
            name: "extensionEnd",
            kind: "extension",
            ref_x: "1",
            ref_y: "7",
            marker_w: "20",
            marker_h: "28",
            shape: MarkerShape::Path("M 1,1 V 13 L18,7 Z"),
        },
    );

    marker(
        &mut ctx,
        MarkerSpec {
            name: "compositionStart",
            kind: "composition",
            ref_x: "18",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            shape: MarkerShape::Path("M 18,7 L9,13 L1,7 L9,1 Z"),
        },
    );
    marker(
        &mut ctx,
        MarkerSpec {
            name: "compositionEnd",
            kind: "composition",
            ref_x: "1",
            ref_y: "7",
            marker_w: "20",
            marker_h: "28",
            shape: MarkerShape::Path("M 18,7 L9,13 L1,7 L9,1 Z"),
        },
    );

    marker(
        &mut ctx,
        MarkerSpec {
            name: "dependencyStart",
            kind: "dependency",
            ref_x: "6",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            shape: MarkerShape::Path("M 5,7 L9,13 L1,7 L9,1 Z"),
        },
    );
    marker(
        &mut ctx,
        MarkerSpec {
            name: "dependencyEnd",
            kind: "dependency",
            ref_x: "13",
            ref_y: "7",
            marker_w: "20",
            marker_h: "28",
            shape: MarkerShape::Path("M 18,7 L9,13 L14,7 L9,1 Z"),
        },
    );

    marker(
        &mut ctx,
        MarkerSpec {
            name: "lollipopStart",
            kind: "lollipop",
            ref_x: "13",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            shape: MarkerShape::Circle,
        },
    );
    marker(
        &mut ctx,
        MarkerSpec {
            name: "lollipopEnd",
            kind: "lollipop",
            ref_x: "1",
            ref_y: "7",
            marker_w: "190",
            marker_h: "240",
            shape: MarkerShape::Circle,
        },
    );
}
