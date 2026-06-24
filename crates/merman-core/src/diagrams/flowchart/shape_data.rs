use super::{Node, TitleKind};
use serde_json::Value;

pub(super) fn parse_shape_data(input: &str) -> std::result::Result<Value, String> {
    crate::inline_config::parse_mermaid_inline_object(input)
}

const MERMAID_SHAPES_11_12_2: &[&str] = &[
    "anchor",
    "bang",
    "bolt",
    "bow-rect",
    "bow-tie-rectangle",
    "brace",
    "brace-l",
    "brace-r",
    "braces",
    "card",
    "choice",
    "circ",
    "circle",
    "classBox",
    "cloud",
    "collate",
    "com-link",
    "comment",
    "cross-circ",
    "crossed-circle",
    "curv-trap",
    "curved-trapezoid",
    "cyl",
    "cylinder",
    "das",
    "data-store",
    "database",
    "datastore",
    "db",
    "dbl-circ",
    "decision",
    "defaultMindmapNode",
    "delay",
    "diam",
    "diamond",
    "disk",
    "display",
    "div-proc",
    "div-rect",
    "divided-process",
    "divided-rectangle",
    "doc",
    "docs",
    "document",
    "documents",
    "double-circle",
    "doublecircle",
    "erBox",
    "event",
    "extract",
    "f-circ",
    "filled-circle",
    "flag",
    "flip-tri",
    "flipped-triangle",
    "fork",
    "forkJoin",
    "fr-circ",
    "fr-rect",
    "framed-circle",
    "framed-rectangle",
    "h-cyl",
    "half-rounded-rectangle",
    "hex",
    "hexagon",
    "horizontal-cylinder",
    "hourglass",
    "icon",
    "iconCircle",
    "iconRounded",
    "iconSquare",
    "imageSquare",
    "in-out",
    "internal-storage",
    "inv-trapezoid",
    "inv_trapezoid",
    "join",
    "junction",
    "kanbanItem",
    "labelRect",
    "lean-l",
    "lean-left",
    "lean-r",
    "lean-right",
    "lean_left",
    "lean_right",
    "lightning-bolt",
    "lin-cyl",
    "lin-doc",
    "lin-proc",
    "lin-rect",
    "lined-cylinder",
    "lined-document",
    "lined-process",
    "lined-rectangle",
    "loop-limit",
    "manual",
    "manual-file",
    "manual-input",
    "mindmapCircle",
    "notch-pent",
    "notch-rect",
    "notched-pentagon",
    "notched-rectangle",
    "note",
    "odd",
    "out-in",
    "paper-tape",
    "pill",
    "prepare",
    "priority",
    "proc",
    "process",
    "processes",
    "procs",
    "question",
    "rect",
    "rectWithTitle",
    "rect_left_inv_arrow",
    "rectangle",
    "requirementBox",
    "rounded",
    "roundedRect",
    "shaded-process",
    "sl-rect",
    "sloped-rectangle",
    "sm-circ",
    "small-circle",
    "squareRect",
    "st-doc",
    "st-rect",
    "stacked-document",
    "stacked-rectangle",
    "stadium",
    "start",
    "state",
    "stateEnd",
    "stateStart",
    "stop",
    "stored-data",
    "subproc",
    "subprocess",
    "subroutine",
    "summary",
    "tag-doc",
    "tag-proc",
    "tag-rect",
    "tagged-document",
    "tagged-process",
    "tagged-rectangle",
    "terminal",
    "text",
    "trap-b",
    "trap-t",
    "trapezoid",
    "trapezoid-bottom",
    "trapezoid-top",
    "tri",
    "triangle",
    "win-pane",
    "window-pane",
];

fn is_valid_shape_11_12_2(shape: &str) -> bool {
    MERMAID_SHAPES_11_12_2.binary_search(&shape).is_ok()
}

pub(super) fn value_to_string(v: &Value) -> Option<String> {
    crate::inline_config::value_to_string(v)
}

pub(super) fn value_to_bool(v: &Value) -> Option<bool> {
    crate::inline_config::value_to_bool(v)
}

fn value_to_f64(v: &Value) -> Option<f64> {
    crate::inline_config::value_to_f64(v)
}

fn sanitize_shape_label_type(label_type: Option<&str>) -> TitleKind {
    match label_type {
        Some("text") => TitleKind::Text,
        Some("string") => TitleKind::String,
        Some("markdown") => TitleKind::Markdown,
        _ => TitleKind::Markdown,
    }
}

pub(super) fn apply_shape_data_to_node(
    node: &mut Node,
    yaml_body: &str,
) -> std::result::Result<(), String> {
    // If shapeData is attached to a node reference, Mermaid has already decided this is a node.
    let v = parse_shape_data(yaml_body)?;
    let map = match v.as_object() {
        Some(m) => m,
        None => return Ok(()),
    };

    let mut provided_label: Option<String> = None;
    let mut provided_label_type: Option<TitleKind> = None;
    for (k, v) in map {
        match k.as_str() {
            "shape" => {
                let Some(shape) = v.as_str() else { continue };
                if shape != shape.to_lowercase() || shape.contains('_') {
                    return Err(format!(
                        "No such shape: {shape}. Shape names should be lowercase."
                    ));
                }
                if !is_valid_shape_11_12_2(shape) {
                    return Err(format!("No such shape: {shape}."));
                }
                node.shape = Some(shape.to_string());
            }
            "label" => {
                if let Some(label) = value_to_string(v) {
                    provided_label = Some(label.clone());
                    node.label = Some(label);
                    node.label_span = None;
                    node.label_selection = None;
                }
            }
            "labelType" => {
                provided_label_type =
                    Some(sanitize_shape_label_type(value_to_string(v).as_deref()));
            }
            "icon" => {
                if let Some(icon) = value_to_string(v) {
                    node.icon = Some(icon);
                }
            }
            "form" => {
                if let Some(form) = value_to_string(v) {
                    node.form = Some(form);
                }
            }
            "pos" => {
                if let Some(pos) = value_to_string(v) {
                    node.pos = Some(pos);
                }
            }
            "img" => {
                if let Some(img) = value_to_string(v) {
                    node.img = Some(img);
                }
            }
            "constraint" => {
                if let Some(constraint) = value_to_string(v) {
                    node.constraint = Some(constraint);
                }
            }
            "w" => {
                if let Some(w) = value_to_f64(v) {
                    node.asset_width = Some(w);
                }
            }
            "h" => {
                if let Some(h) = value_to_f64(v) {
                    node.asset_height = Some(h);
                }
            }
            _ => {}
        }
    }
    if provided_label.is_some() {
        node.label_type = provided_label_type.unwrap_or(TitleKind::Markdown);
    }

    // Mermaid clears the default label when an icon or img is set without an explicit label.
    let has_visual = node.icon.is_some() || node.img.is_some();
    let label_is_empty_or_missing = provided_label
        .as_deref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if has_visual && label_is_empty_or_missing {
        let current_text = node.label.as_deref().unwrap_or(node.id.as_str());
        if current_text == node.id {
            node.label = Some(String::new());
            node.label_type = TitleKind::Text;
            node.label_span = None;
            node.label_selection = None;
        }
    }

    Ok(())
}
