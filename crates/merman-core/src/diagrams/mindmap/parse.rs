use serde_json::{Map, Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Error, ParseMetadata, Result};

use super::db::MindmapDb;
use super::render_model::MindmapDiagramRenderModel;
use super::utils::{
    parse_node_spec, split_indent, starts_with_case_insensitive, strip_inline_comment,
};

static MINDMAP_DIAGRAM_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn parse_mindmap(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_mindmap_impl(code, meta, MindmapEmit::Full)
}

pub fn parse_mindmap_for_render(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_mindmap_impl(code, meta, MindmapEmit::RenderOnly)
}

pub fn parse_mindmap_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<MindmapDiagramRenderModel> {
    let mut db = parse_mindmap_db(code, meta)?;
    let Some(root_id) = db.get_mindmap().map(|n| n.id) else {
        return Ok(MindmapDiagramRenderModel::default());
    };

    db.assign_sections(root_id, None);

    Ok(MindmapDiagramRenderModel {
        nodes: db.to_layout_nodes_for_render(root_id),
        edges: db.to_edges_for_render(root_id),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MindmapEmit {
    Full,
    RenderOnly,
}

fn parse_mindmap_db(code: &str, meta: &ParseMetadata) -> Result<MindmapDb> {
    let mut db = MindmapDb::default();
    db.clear();

    let mut lines = code.lines();
    let mut found_header = false;
    let mut header_tail: Option<String> = None;
    for line in lines.by_ref() {
        let t = strip_inline_comment(line);
        let trimmed = t.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("mindmap") {
            found_header = true;
            break;
        }
        if starts_with_case_insensitive(trimmed, "mindmap")
            && trimmed.len() > "mindmap".len()
            && trimmed["mindmap".len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace())
        {
            found_header = true;
            let after_keyword = &trimmed["mindmap".len()..];
            let indent = after_keyword
                .chars()
                .take_while(|c| c.is_whitespace())
                .count();
            let rest = after_keyword.trim_start();
            if !rest.is_empty() {
                header_tail = Some(format!("{}{}", " ".repeat(indent), rest));
            }
            break;
        }
        break;
    }

    if !found_header {
        return Err(Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: "expected mindmap header".to_string(),
        });
    }

    enum HandleOutcome {
        Done,
        NeedMoreInput,
    }

    let mut handle_line = |line: &str| -> Result<HandleOutcome> {
        if line.trim().is_empty() {
            return Ok(HandleOutcome::Done);
        }

        let (indent, rest) = split_indent(line);
        let rest = rest.trim_end();
        if rest.is_empty() {
            return Ok(HandleOutcome::Done);
        }

        if starts_with_case_insensitive(rest, "::icon(") {
            let after = &rest["::icon(".len()..];
            let Some(end) = after.find(')') else {
                return Ok(HandleOutcome::Done);
            };
            let icon = after[..end].to_string();
            db.decorate_last(None, Some(icon), &meta.effective_config);
            return Ok(HandleOutcome::Done);
        }

        if let Some(after) = rest.strip_prefix(":::") {
            // Mermaid mindmap does not treat `%% ...` as an inline comment inside `:::` class
            // directives (the entire remainder is interpreted as space-separated class names).
            db.decorate_last(Some(after.trim().to_string()), None, &meta.effective_config);
            return Ok(HandleOutcome::Done);
        }

        let rest = strip_inline_comment(rest).trim_end();
        if rest.is_empty() {
            return Ok(HandleOutcome::Done);
        }

        let (id_raw, descr_raw, ty, descr_is_markdown) = match parse_node_spec(rest) {
            Ok(v) => v,
            Err(message) if message == "unterminated node delimiter" => {
                return Ok(HandleOutcome::NeedMoreInput);
            }
            Err(message) => {
                return Err(Error::DiagramParse {
                    diagram_type: meta.diagram_type.clone(),
                    message,
                });
            }
        };
        db.add_node(
            indent as i32,
            &id_raw,
            &descr_raw,
            descr_is_markdown,
            ty,
            &meta.diagram_type,
            &meta.effective_config,
        )?;
        Ok(HandleOutcome::Done)
    };

    let mut pending: Option<String> = None;
    let mut push_and_try = |physical_line: &str| -> Result<()> {
        match pending.as_mut() {
            Some(buf) => {
                buf.push('\n');
                buf.push_str(physical_line);
            }
            None => pending = Some(physical_line.to_string()),
        }

        let current = pending.as_deref().unwrap_or_default();
        match handle_line(current)? {
            HandleOutcome::Done => {
                pending = None;
            }
            HandleOutcome::NeedMoreInput => {}
        }
        Ok(())
    };

    if let Some(tail) = &header_tail {
        push_and_try(tail)?;
    }
    for line in lines {
        push_and_try(line)?;
    }
    if let Some(buf) = pending {
        let line = strip_inline_comment(&buf);
        if !line.trim().is_empty() {
            return Err(Error::DiagramParse {
                diagram_type: meta.diagram_type.clone(),
                message: "unterminated node delimiter".to_string(),
            });
        }
    }

    Ok(db)
}

fn parse_mindmap_impl(code: &str, meta: &ParseMetadata, emit: MindmapEmit) -> Result<Value> {
    let mut db = parse_mindmap_db(code, meta)?;

    let Some(root_id) = db.get_mindmap().map(|n| n.id) else {
        if emit == MindmapEmit::Full {
            let mut final_config = meta.effective_config.as_value().clone();
            if meta.config.as_value().get("layout").is_none() {
                if let Some(obj) = final_config.as_object_mut() {
                    obj.insert(
                        "layout".to_string(),
                        Value::String("cose-bilkent".to_string()),
                    );
                }
            }

            return Ok(json!({
                "nodes": [],
                "edges": [],
                "config": final_config,
            }));
        }

        return Ok(json!({
            "nodes": [],
            "edges": [],
        }));
    };

    db.assign_sections(root_id, None);

    let nodes = db.to_layout_node_values(root_id);
    let edges = db.to_edge_values(root_id);

    if emit == MindmapEmit::RenderOnly {
        return Ok(json!({
            "nodes": nodes,
            "edges": edges,
        }));
    }

    let mut final_config = meta.effective_config.as_value().clone();
    if meta.config.as_value().get("layout").is_none() {
        if let Some(obj) = final_config.as_object_mut() {
            obj.insert(
                "layout".to_string(),
                Value::String("cose-bilkent".to_string()),
            );
        }
    }

    let mut shapes = Map::new();
    for n in nodes.iter() {
        let Some(node) = n.as_object() else {
            continue;
        };
        let Some(id) = node.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let shape = node.get("shape").cloned().unwrap_or(Value::Null);
        let width = node.get("width").cloned().unwrap_or(Value::Null);
        let height = node.get("height").cloned().unwrap_or(Value::Null);
        let padding = node.get("padding").cloned().unwrap_or(Value::Null);
        shapes.insert(
            id.to_string(),
            json!({
                "shape": shape,
                "width": width,
                "height": height,
                "padding": padding,
            }),
        );
    }

    let diagram_id = MINDMAP_DIAGRAM_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

    Ok(json!({
        "type": meta.diagram_type,
        "nodes": nodes,
        "edges": edges,
        "config": final_config,
        "rootNode": db.to_root_node_value(root_id),
        "markers": ["point"],
        "direction": "TB",
        "nodeSpacing": 50,
        "rankSpacing": 50,
        "shapes": Value::Object(shapes),
        // Mermaid uses a random UUID v4 here. For performance and determinism, keep a cheap
        // monotonic id that is unique within the current process. Snapshot tests normalize this
        // field to "<dynamic>".
        "diagramId": format!("mindmap-{diagram_id}"),
    }))
}
