//! Shared Mermaid edge-marker DOM emission.
//!
//! Mermaid's rendering-elements marker helper is used by several diagram families.  Keep its
//! base marker set outside a family-specific module so Block and Flowchart cannot drift in marker
//! count, ordering, ids, or SVG attributes.

use std::fmt::Write as _;

use super::SvgDiagramId;
use super::util::escape_xml;

pub(in crate::svg::parity) fn push_base_edge_markers(
    out: &mut String,
    diagram_id: SvgDiagramId<'_>,
    diagram_type: &str,
) {
    let ty = escape_xml(diagram_type);
    let _ = write!(
        out,
        r#"<marker id="{}_{}-pointEnd" class="marker {}" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-pointStart" class="marker {}" viewBox="0 0 10 10" refX="4.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 5 L 10 10 L 10 0 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-pointEnd-margin" class="marker {}" viewBox="0 0 11.5 14" refX="11.5" refY="7" markerUnits="userSpaceOnUse" markerWidth="10.5" markerHeight="14" orient="auto"><path d="M 0 0 L 11.5 7 L 0 14 z" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-pointStart-margin" class="marker {}" viewBox="0 0 11.5 14" refX="1" refY="7" markerUnits="userSpaceOnUse" markerWidth="11.5" markerHeight="14" orient="auto"><polygon points="0,7 11.5,14 11.5,0" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-circleEnd" class="marker {}" viewBox="0 0 10 10" refX="11" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-circleStart" class="marker {}" viewBox="0 0 10 10" refX="-1" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-circleEnd-margin" class="marker {}" viewBox="0 0 10 10" refY="5" refX="12.25" markerUnits="userSpaceOnUse" markerWidth="14" markerHeight="14" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-circleStart-margin" class="marker {}" viewBox="0 0 10 10" refX="-2" refY="5" markerUnits="userSpaceOnUse" markerWidth="14" markerHeight="14" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-crossEnd" class="marker cross {}" viewBox="0 0 11 11" refX="12" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-crossStart" class="marker cross {}" viewBox="0 0 11 11" refX="-1" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-crossEnd-margin" class="marker cross {}" viewBox="0 0 15 15" refX="17.7" refY="7.5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 1,1 L 14,14 M 1,14 L 14,1" class="arrowMarkerPath" style="stroke-width: 2.5;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
    let _ = write!(
        out,
        r#"<marker id="{}_{}-crossStart-margin" class="marker cross {}" viewBox="0 0 15 15" refX="-3.5" refY="7.5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 1,1 L 14,14 M 1,14 L 14,1" class="arrowMarkerPath" style="stroke-width: 2.5; stroke-dasharray: 1, 0;"/></marker>"#,
        diagram_id,
        ty.as_str(),
        ty.as_str()
    );
}
