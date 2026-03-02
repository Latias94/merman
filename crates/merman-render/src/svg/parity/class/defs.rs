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
    fn marker_path(
        out: &mut String,
        diagram_id: &str,
        diagram_marker_class: &str,
        name: &str,
        kind: &str,
        ref_x: &str,
        ref_y: &str,
        marker_w: &str,
        marker_h: &str,
        d: &str,
    ) {
        let _ = write!(
            out,
            r#"<defs><marker id="{}_{}-{}" class="marker {} {}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto"><path d="{}"/></marker></defs>"#,
            escape_xml_display(diagram_id),
            escape_xml_display(diagram_marker_class),
            escape_xml_display(name),
            escape_xml_display(kind),
            escape_xml_display(diagram_marker_class),
            ref_x,
            ref_y,
            marker_w,
            marker_h,
            escape_xml_display(d)
        );
    }

    fn marker_circle(
        out: &mut String,
        diagram_id: &str,
        diagram_marker_class: &str,
        name: &str,
        kind: &str,
        ref_x: &str,
        ref_y: &str,
        marker_w: &str,
        marker_h: &str,
    ) {
        let _ = write!(
            out,
            r#"<defs><marker id="{}_{}-{}" class="marker {} {}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto"><circle stroke="black" fill="transparent" cx="7" cy="7" r="6"/></marker></defs>"#,
            escape_xml_display(diagram_id),
            escape_xml_display(diagram_marker_class),
            escape_xml_display(name),
            escape_xml_display(kind),
            escape_xml_display(diagram_marker_class),
            ref_x,
            ref_y,
            marker_w,
            marker_h
        );
    }

    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "aggregationStart",
        "aggregation",
        "18",
        "7",
        "190",
        "240",
        "M 18,7 L9,13 L1,7 L9,1 Z",
    );
    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "aggregationEnd",
        "aggregation",
        "1",
        "7",
        "20",
        "28",
        "M 18,7 L9,13 L1,7 L9,1 Z",
    );

    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "extensionStart",
        "extension",
        "18",
        "7",
        "190",
        "240",
        "M 1,7 L18,13 V 1 Z",
    );
    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "extensionEnd",
        "extension",
        "1",
        "7",
        "20",
        "28",
        "M 1,1 V 13 L18,7 Z",
    );

    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "compositionStart",
        "composition",
        "18",
        "7",
        "190",
        "240",
        "M 18,7 L9,13 L1,7 L9,1 Z",
    );
    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "compositionEnd",
        "composition",
        "1",
        "7",
        "20",
        "28",
        "M 18,7 L9,13 L1,7 L9,1 Z",
    );

    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "dependencyStart",
        "dependency",
        "6",
        "7",
        "190",
        "240",
        "M 5,7 L9,13 L1,7 L9,1 Z",
    );
    marker_path(
        out,
        diagram_id,
        diagram_marker_class,
        "dependencyEnd",
        "dependency",
        "13",
        "7",
        "20",
        "28",
        "M 18,7 L9,13 L14,7 L9,1 Z",
    );

    marker_circle(
        out,
        diagram_id,
        diagram_marker_class,
        "lollipopStart",
        "lollipop",
        "13",
        "7",
        "190",
        "240",
    );
    marker_circle(
        out,
        diagram_id,
        diagram_marker_class,
        "lollipopEnd",
        "lollipop",
        "1",
        "7",
        "190",
        "240",
    );
}
