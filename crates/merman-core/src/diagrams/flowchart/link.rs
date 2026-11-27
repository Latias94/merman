#[cfg(all(test, feature = "diagram-flowchart"))]
mod tests {
    use crate::{SemanticDiagramModel, SemanticParser};
    use merman_model::families::flowchart::FlowEdgeMarker::{Circle, Cross, None as Open, Point};

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
