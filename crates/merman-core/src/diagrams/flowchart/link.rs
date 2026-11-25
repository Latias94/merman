use super::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, LinkToken};

pub(super) fn compatibility_edge_type(link: &LinkToken) -> &'static str {
    // An open START_LINK inherits the final LINK's aggregate type, including a pair
    // such as `o--o` captured at the end of `--No--o`. Keep the authored marker for ASCII.
    let start_marker = match link.start_marker {
        FlowEdgeMarker::None => link.final_operator_start_marker(),
        marker => marker,
    };
    match (start_marker, link.end_marker) {
        (FlowEdgeMarker::Point, FlowEdgeMarker::Point) => "double_arrow_point",
        (FlowEdgeMarker::Circle, FlowEdgeMarker::Circle) => "double_arrow_circle",
        (FlowEdgeMarker::Cross, FlowEdgeMarker::Cross) => "double_arrow_cross",
        (_, FlowEdgeMarker::Point) => "arrow_point",
        (_, FlowEdgeMarker::Circle) => "arrow_circle",
        (_, FlowEdgeMarker::Cross) => "arrow_cross",
        (_, FlowEdgeMarker::None) => "arrow_open",
    }
}

pub(super) const fn compatibility_stroke(link: &LinkToken) -> &'static str {
    if matches!(link.visibility, FlowEdgeVisibility::Invisible) {
        return "invisible";
    }
    match link.stroke_kind {
        FlowEdgeStroke::Normal => "normal",
        FlowEdgeStroke::Dotted => "dotted",
        FlowEdgeStroke::Thick => "thick",
    }
}

#[cfg(all(test, feature = "diagram-flowchart"))]
mod tests {
    use super::FlowEdgeMarker::{Circle, Cross, None as Open, Point};
    use crate::{SemanticDiagramModel, SemanticParser};

    #[test]
    fn link_markers_preserve_mermaid_stroke_and_length() {
        let parser = SemanticParser::new();
        // FlowDB.getEdges() after flow.jison parses each operator between A and B.
        for (operator, compatibility, markers, label) in [
            ("o-->", ("arrow_point", "normal", 2), (Circle, Point), None),
            ("o==>", ("arrow_point", "normal", 2), (Circle, Point), None),
            (
                "x===o",
                ("arrow_circle", "normal", 3),
                (Cross, Circle),
                None,
            ),
            ("o---", ("arrow_open", "normal", 2), (Circle, Open), None),
            ("<===", ("arrow_open", "normal", 2), (Point, Open), None),
            ("==o", ("arrow_circle", "thick", 1), (Open, Circle), None),
            (
                "o==o",
                ("double_arrow_circle", "thick", 1),
                (Circle, Circle),
                None,
            ),
            (
                "o-...-x",
                ("arrow_cross", "dotted", 3),
                (Circle, Cross),
                None,
            ),
            (
                "o------------>",
                ("arrow_point", "normal", 10),
                (Circle, Point),
                None,
            ),
            (
                "o==>|go|",
                ("arrow_point", "normal", 2),
                (Circle, Point),
                Some("go"),
            ),
            (
                "--No-->",
                ("arrow_point", "normal", 2),
                (Open, Point),
                Some("N"),
            ),
            (
                "--No--o",
                ("double_arrow_circle", "normal", 1),
                (Open, Circle),
                Some("N"),
            ),
            (
                "==Box==x",
                ("double_arrow_cross", "thick", 1),
                (Open, Cross),
                Some("Bo"),
            ),
            (
                "o== go ==o",
                ("double_arrow_circle", "thick", 1),
                (Circle, Circle),
                Some("go"),
            ),
        ] {
            let source = format!("flowchart LR\nA {operator} B");
            let parsed = parser
                .parse_diagram_sync(&source)
                .unwrap_or_else(|e| panic!("{source}: {e}"));
            let SemanticDiagramModel::Flowchart(model) = parsed.model else {
                panic!("expected Flowchart for {source}");
            };
            let [edge] = model.edges.as_slice() else {
                panic!("expected one edge for {source}");
            };
            let (edge_type, stroke, length) = compatibility;
            assert_eq!(edge.edge_type.as_deref(), Some(edge_type), "{source}");
            assert_eq!(edge.stroke.as_deref(), Some(stroke), "{source}");
            assert_eq!(edge.length, length, "{source}");
            assert_eq!((edge.start_marker, edge.end_marker), markers, "{source}");
            assert_eq!(edge.label.as_deref(), label, "{source}");
        }
    }
}
