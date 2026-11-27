#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use super::LinkToken;
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

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
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

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
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
