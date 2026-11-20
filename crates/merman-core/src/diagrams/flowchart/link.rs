use super::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StartLinkSemantics {
    pub marker: FlowEdgeMarker,
    pub stroke_kind: FlowEdgeStroke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EndLinkSemantics {
    pub start_marker: FlowEdgeMarker,
    pub end_marker: FlowEdgeMarker,
    pub stroke_kind: FlowEdgeStroke,
    pub visibility: FlowEdgeVisibility,
    pub length: usize,
}

pub(super) fn operator_start_marker(operator: &str) -> FlowEdgeMarker {
    match operator.as_bytes().first() {
        Some(b'<') => FlowEdgeMarker::Point,
        Some(b'x') => FlowEdgeMarker::Cross,
        Some(b'o') => FlowEdgeMarker::Circle,
        _ => FlowEdgeMarker::None,
    }
}

pub(super) fn destruct_start_link(s: &str) -> StartLinkSemantics {
    let line = s.trim();
    let stroke_kind = if line.contains('.') {
        FlowEdgeStroke::Dotted
    } else if line.contains('=') {
        FlowEdgeStroke::Thick
    } else {
        FlowEdgeStroke::Normal
    };
    StartLinkSemantics {
        marker: operator_start_marker(line),
        stroke_kind,
    }
}

pub(super) fn destruct_end_link(s: &str) -> EndLinkSemantics {
    let line = s.trim();
    if line.len() < 2 {
        return EndLinkSemantics {
            start_marker: FlowEdgeMarker::None,
            end_marker: FlowEdgeMarker::None,
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Visible,
            length: 1,
        };
    }

    let start_marker = operator_start_marker(line);
    let end_marker = match line.as_bytes().last().copied() {
        Some(b'>') => FlowEdgeMarker::Point,
        Some(b'x') => FlowEdgeMarker::Cross,
        Some(b'o') => FlowEdgeMarker::Circle,
        _ => FlowEdgeMarker::None,
    };

    // FlowDB.destructEndLink drops the final character of every LINK and the first character
    // only for a matching marker pair. An unpaired prefix counts toward length and hides `=`.
    let mut stroke = &line.as_bytes()[..line.len() - 1];
    if start_marker != FlowEdgeMarker::None && start_marker == end_marker {
        stroke = &stroke[1..];
    }

    let visibility = if stroke.starts_with(b"~") {
        FlowEdgeVisibility::Invisible
    } else {
        FlowEdgeVisibility::Visible
    };
    let mut stroke_kind = if stroke.starts_with(b"=") {
        FlowEdgeStroke::Thick
    } else {
        FlowEdgeStroke::Normal
    };
    let mut length = stroke.len().saturating_sub(1);
    let dots = stroke.iter().filter(|&&byte| byte == b'.').count();
    if dots > 0 {
        stroke_kind = FlowEdgeStroke::Dotted;
        length = dots;
    }

    EndLinkSemantics {
        start_marker,
        end_marker,
        stroke_kind,
        visibility,
        length,
    }
}

pub(super) fn destruct_labeled_end_link(s: &str) -> EndLinkSemantics {
    // In flow.jison's `START_LINK edgeText LINK`, START_LINK owns the authored source marker.
    // FlowDB still measures the entire final LINK, including any character it took from the label.
    EndLinkSemantics {
        start_marker: FlowEdgeMarker::None,
        ..destruct_end_link(s)
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
