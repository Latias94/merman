use crate::sanitize::sanitize_text;
use crate::{Error, MermaidConfig, ParseMetadata, Result};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

#[derive(Debug, Default)]
struct C4Db {
    c4_type: String,
    title: String,
    acc_descr: String,
    wrap_enabled: bool,

    current_boundary: String,
    parent_boundary: String,
    boundary_stack: Vec<String>,

    boundaries: Vec<Map<String, Value>>,
    boundary_index: HashMap<String, usize>,

    shapes: Vec<Map<String, Value>>,
    shape_index: HashMap<String, usize>,

    rels: Vec<Map<String, Value>>,

    c4_shape_in_row: i64,
    c4_boundary_in_row: i64,
}

pub fn parse_c4(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_c4_impl(code, meta)
}

impl C4Db {
    fn new(config: &MermaidConfig) -> Self {
        let wrap_enabled = config.get_bool("wrap").unwrap_or(false);
        let mut db = Self {
            wrap_enabled,
            current_boundary: "global".to_string(),
            c4_shape_in_row: 4,
            c4_boundary_in_row: 2,
            ..Default::default()
        };
        db.boundary_stack.push(String::new());
        db.add_global_boundary();
        db
    }

    fn add_global_boundary(&mut self) {
        let mut obj = Map::new();
        obj.insert("alias".to_string(), json!("global"));
        obj.insert("label".to_string(), wrap_text(json!("global")));
        obj.insert("type".to_string(), wrap_text(json!("global")));
        obj.insert("tags".to_string(), Value::Null);
        obj.insert("link".to_string(), Value::Null);
        obj.insert("parentBoundary".to_string(), json!(""));
        self.boundary_index
            .insert("global".to_string(), self.boundaries.len());
        self.boundaries.push(obj);
    }

    fn set_c4_type(&mut self, raw: &str, config: &MermaidConfig) {
        self.c4_type = sanitize_text(raw, config);
    }

    fn set_title(&mut self, raw: &str, config: &MermaidConfig) {
        self.title = sanitize_text(raw, config);
    }

    fn set_acc_description(&mut self, raw: &str) {
        self.acc_descr = raw.to_string();
    }

    fn add_person_or_system(&mut self, type_c4_shape: &str, args: &[Value]) -> Result<()> {
        let alias = arg_to_string(args.get(0))?;
        let label = args.get(1).cloned().unwrap_or_else(|| json!(""));
        let descr = args.get(2).cloned();

        let idx = match self.shape_index.get(&alias).copied() {
            Some(idx) => idx,
            None => {
                let idx = self.shapes.len();
                self.shape_index.insert(alias.clone(), idx);
                let mut obj = Map::new();
                obj.insert("alias".to_string(), json!(alias));
                self.shapes.push(obj);
                idx
            }
        };

        let obj = self.shapes.get_mut(idx).unwrap();
        obj.insert("label".to_string(), wrap_text(label));
        apply_text_field_or_kv(obj, "descr", descr.unwrap_or_else(|| json!("")))?;
        obj.insert("typeC4Shape".to_string(), wrap_text(json!(type_c4_shape)));
        obj.insert(
            "parentBoundary".to_string(),
            json!(self.current_boundary.clone()),
        );
        obj.insert("wrap".to_string(), json!(self.wrap_enabled));

        apply_kv_value(obj, "sprite", args.get(3))?;
        apply_kv_value(obj, "tags", args.get(4))?;
        apply_kv_value(obj, "link", args.get(5))?;
        Ok(())
    }

    fn add_container(&mut self, type_c4_shape: &str, args: &[Value]) -> Result<()> {
        let alias = arg_to_string(args.get(0))?;
        let label = args.get(1).cloned().unwrap_or_else(|| json!(""));
        let techn = args.get(2).cloned();
        let descr = args.get(3).cloned();

        let idx = match self.shape_index.get(&alias).copied() {
            Some(idx) => idx,
            None => {
                let idx = self.shapes.len();
                self.shape_index.insert(alias.clone(), idx);
                let mut obj = Map::new();
                obj.insert("alias".to_string(), json!(alias));
                self.shapes.push(obj);
                idx
            }
        };

        let obj = self.shapes.get_mut(idx).unwrap();
        obj.insert("label".to_string(), wrap_text(label));
        apply_text_field_or_kv(obj, "techn", techn.unwrap_or_else(|| json!("")))?;
        apply_text_field_or_kv(obj, "descr", descr.unwrap_or_else(|| json!("")))?;
        obj.insert("typeC4Shape".to_string(), wrap_text(json!(type_c4_shape)));
        obj.insert(
            "parentBoundary".to_string(),
            json!(self.current_boundary.clone()),
        );
        obj.insert("wrap".to_string(), json!(self.wrap_enabled));

        apply_kv_value(obj, "sprite", args.get(4))?;
        apply_kv_value(obj, "tags", args.get(5))?;
        apply_kv_value(obj, "link", args.get(6))?;
        Ok(())
    }

    fn add_component(&mut self, type_c4_shape: &str, args: &[Value]) -> Result<()> {
        self.add_container(type_c4_shape, args)
    }

    fn add_person_or_system_boundary(&mut self, args: Vec<Value>) -> Result<()> {
        let alias = arg_to_string(args.get(0))?;
        let label = args.get(1).cloned().unwrap_or_else(|| json!(""));
        let boundary_type = args.get(2).cloned();
        let tags = args.get(3).cloned();
        let link = args.get(4).cloned();

        let idx = match self.boundary_index.get(&alias).copied() {
            Some(idx) => idx,
            None => {
                let idx = self.boundaries.len();
                self.boundary_index.insert(alias.clone(), idx);
                let mut obj = Map::new();
                obj.insert("alias".to_string(), json!(alias));
                self.boundaries.push(obj);
                idx
            }
        };

        let obj = self.boundaries.get_mut(idx).unwrap();
        obj.insert("label".to_string(), wrap_text(label));
        let ty = boundary_type.unwrap_or_else(|| json!("system"));
        apply_text_field_or_kv(obj, "type", ty)?;

        apply_kv_value(obj, "tags", tags.as_ref())?;
        apply_kv_value(obj, "link", link.as_ref())?;

        obj.insert(
            "parentBoundary".to_string(),
            json!(self.current_boundary.clone()),
        );
        obj.insert("wrap".to_string(), json!(self.wrap_enabled));

        self.parent_boundary = self.current_boundary.clone();
        self.current_boundary = alias;
        self.boundary_stack.push(self.parent_boundary.clone());

        Ok(())
    }

    fn add_container_boundary(&mut self, args: Vec<Value>) -> Result<()> {
        self.add_person_or_system_boundary(args)
    }

    fn add_deployment_node(&mut self, node_type: &str, args: Vec<Value>) -> Result<()> {
        let alias = arg_to_string(args.get(0))?;
        let label = args.get(1).cloned().unwrap_or_else(|| json!(""));
        let node_label_type = args.get(2).cloned();
        let descr = args.get(3).cloned();
        let tags = args.get(5).cloned();
        let link = args.get(6).cloned();

        let idx = match self.boundary_index.get(&alias).copied() {
            Some(idx) => idx,
            None => {
                let idx = self.boundaries.len();
                self.boundary_index.insert(alias.clone(), idx);
                let mut obj = Map::new();
                obj.insert("alias".to_string(), json!(alias));
                self.boundaries.push(obj);
                idx
            }
        };

        let obj = self.boundaries.get_mut(idx).unwrap();
        obj.insert("label".to_string(), wrap_text(label));

        let ty = node_label_type.unwrap_or_else(|| json!("node"));
        apply_text_field_or_kv(obj, "type", ty)?;
        apply_text_field_or_kv(obj, "descr", descr.unwrap_or_else(|| json!("")))?;
        apply_kv_value(obj, "tags", tags.as_ref())?;
        apply_kv_value(obj, "link", link.as_ref())?;

        obj.insert("nodeType".to_string(), json!(node_type));
        obj.insert(
            "parentBoundary".to_string(),
            json!(self.current_boundary.clone()),
        );
        obj.insert("wrap".to_string(), json!(self.wrap_enabled));

        self.parent_boundary = self.current_boundary.clone();
        self.current_boundary = alias;
        self.boundary_stack.push(self.parent_boundary.clone());

        Ok(())
    }

    fn pop_boundary_parse_stack(&mut self) {
        self.current_boundary = self.parent_boundary.clone();
        self.boundary_stack.pop();
        self.parent_boundary = self.boundary_stack.pop().unwrap_or_default();
        self.boundary_stack.push(self.parent_boundary.clone());
    }

    fn add_rel(&mut self, rel_type: &str, args: Vec<Value>) -> Result<()> {
        let from = arg_to_string(args.get(0))?;
        let to = arg_to_string(args.get(1))?;
        let Some(label) = args.get(2).cloned() else {
            return Ok(());
        };

        let existing_idx = self
            .rels
            .iter()
            .position(|r| r.get("from") == Some(&json!(from)) && r.get("to") == Some(&json!(to)));

        let rel = if let Some(idx) = existing_idx {
            self.rels.get_mut(idx).unwrap()
        } else {
            self.rels.push(Map::new());
            self.rels.last_mut().unwrap()
        };

        rel.insert("type".to_string(), json!(rel_type));
        rel.insert("from".to_string(), json!(from));
        rel.insert("to".to_string(), json!(to));
        rel.insert("label".to_string(), wrap_text(label));

        let techn = args.get(3).cloned().unwrap_or_else(|| json!(""));
        apply_text_field_or_kv(rel, "techn", techn)?;
        let descr = args.get(4).cloned().unwrap_or_else(|| json!(""));
        apply_text_field_or_kv(rel, "descr", descr)?;

        apply_kv_value(rel, "sprite", args.get(5))?;
        apply_kv_value(rel, "tags", args.get(6))?;
        apply_kv_value(rel, "link", args.get(7))?;
        rel.insert("wrap".to_string(), json!(self.wrap_enabled));
        Ok(())
    }

    fn update_el_style(&mut self, args: Vec<Value>) -> Result<()> {
        let element_name = arg_to_string(args.get(0))?;
        let Some(target) = self
            .shape_index
            .get(&element_name)
            .and_then(|idx| self.shapes.get_mut(*idx))
            .or_else(|| {
                self.boundary_index
                    .get(&element_name)
                    .and_then(|idx| self.boundaries.get_mut(*idx))
            })
        else {
            return Ok(());
        };

        apply_kv_value(target, "bgColor", args.get(1))?;
        apply_kv_value(target, "fontColor", args.get(2))?;
        apply_kv_value(target, "borderColor", args.get(3))?;
        apply_kv_value(target, "shadowing", args.get(4))?;
        apply_kv_value(target, "shape", args.get(5))?;
        apply_kv_value(target, "sprite", args.get(6))?;
        apply_kv_value(target, "techn", args.get(7))?;
        apply_kv_value(target, "legendText", args.get(8))?;
        apply_kv_value(target, "legendSprite", args.get(9))?;
        Ok(())
    }

    fn update_rel_style(&mut self, args: Vec<Value>) -> Result<()> {
        let from = arg_to_string(args.get(0))?;
        let to = arg_to_string(args.get(1))?;

        let Some(target) = self
            .rels
            .iter_mut()
            .find(|r| r.get("from") == Some(&json!(from)) && r.get("to") == Some(&json!(to)))
        else {
            return Ok(());
        };

        apply_kv_value(target, "textColor", args.get(2))?;
        apply_kv_value(target, "lineColor", args.get(3))?;
        if let Some(v) = args.get(4) {
            apply_int_kv(target, "offsetX", v)?;
        }
        if let Some(v) = args.get(5) {
            apply_int_kv(target, "offsetY", v)?;
        }
        Ok(())
    }

    fn update_layout_config(&mut self, args: Vec<Value>) -> Result<()> {
        if let Some(v) = args.get(0) {
            if let Some(parsed) = value_as_i64(v) {
                if parsed >= 1 {
                    self.c4_shape_in_row = parsed;
                }
            }
        }
        if let Some(v) = args.get(1) {
            if let Some(parsed) = value_as_i64(v) {
                if parsed >= 1 {
                    self.c4_boundary_in_row = parsed;
                }
            }
        }
        Ok(())
    }

    fn to_model(&self, meta: &ParseMetadata) -> Value {
        json!({
            "type": meta.diagram_type,
            "c4Type": self.c4_type,
            "title": if self.title.is_empty() { Value::Null } else { json!(self.title) },
            "accTitle": Value::Null,
            "accDescr": if self.acc_descr.is_empty() { Value::Null } else { json!(self.acc_descr) },
            "wrap": self.wrap_enabled,
            "layout": {
                "c4ShapeInRow": self.c4_shape_in_row,
                "c4BoundaryInRow": self.c4_boundary_in_row,
            },
            "shapes": self.shapes.iter().map(|m| Value::Object(m.clone())).collect::<Vec<_>>(),
            "boundaries": self.boundaries.iter().map(|m| Value::Object(m.clone())).collect::<Vec<_>>(),
            "rels": self.rels.iter().map(|m| Value::Object(m.clone())).collect::<Vec<_>>(),
            "config": meta.effective_config.as_value().clone(),
        })
    }
}

fn wrap_text(v: Value) -> Value {
    json!({ "text": v })
}

fn arg_to_string(v: Option<&Value>) -> Result<String> {
    match v {
        None => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: format!("expected string argument, got {other:?}"),
        }),
    }
}

fn apply_text_field_or_kv(obj: &mut Map<String, Value>, default_key: &str, v: Value) -> Result<()> {
    match v {
        Value::Object(map) => {
            let mut iter = map.into_iter();
            let Some((k, vv)) = iter.next() else {
                obj.insert(default_key.to_string(), wrap_text(json!("")));
                return Ok(());
            };
            let s = match vv {
                Value::String(s) => s,
                other => {
                    return Err(Error::DiagramParse {
                        diagram_type: "c4".to_string(),
                        message: format!("expected string in attribute kv, got {other:?}"),
                    });
                }
            };
            obj.insert(k, wrap_text(json!(s)));
            Ok(())
        }
        Value::String(s) => {
            obj.insert(default_key.to_string(), wrap_text(json!(s)));
            Ok(())
        }
        other => Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: format!("invalid text field value: {other:?}"),
        }),
    }
}

fn apply_kv_value(
    obj: &mut Map<String, Value>,
    default_key: &str,
    v: Option<&Value>,
) -> Result<()> {
    let Some(v) = v else {
        return Ok(());
    };

    match v {
        Value::Object(map) => {
            let mut iter = map.clone().into_iter();
            let Some((k, vv)) = iter.next() else {
                return Ok(());
            };
            obj.insert(k, vv);
            Ok(())
        }
        Value::String(s) => {
            obj.insert(default_key.to_string(), json!(s));
            Ok(())
        }
        other => Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: format!("invalid kv value: {other:?}"),
        }),
    }
}

fn apply_int_kv(obj: &mut Map<String, Value>, key: &str, v: &Value) -> Result<()> {
    let Some(parsed) = value_as_i64(v) else {
        return Ok(());
    };
    obj.insert(key.to_string(), json!(parsed));
    Ok(())
}

fn value_as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_u64().map(|v| v as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        Value::Object(map) => map.values().next().and_then(value_as_i64),
        _ => None,
    }
}

fn parse_c4_impl(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut db = C4Db::new(&meta.effective_config);

    let mut lines = code.lines().peekable();
    let header = next_non_empty_line(&mut lines).ok_or_else(|| Error::DiagramParse {
        diagram_type: meta.diagram_type.clone(),
        message: "expected C4 header".to_string(),
    })?;
    let header = header.trim();

    match header {
        "C4Context" | "C4Container" | "C4Component" | "C4Dynamic" | "C4Deployment" => {}
        _ => {
            return Err(Error::DiagramParse {
                diagram_type: meta.diagram_type.clone(),
                message: format!("unexpected C4 header: {header}"),
            });
        }
    }
    db.set_c4_type(header, &meta.effective_config);

    while let Some(raw) = lines.next() {
        let raw = strip_inline_comment(raw);
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        if t == "}" {
            db.pop_boundary_parse_stack();
            continue;
        }

        if let Some(title) = try_parse_title(t) {
            db.set_title(&title, &meta.effective_config);
            continue;
        }

        if let Some(acc) = try_parse_acc_description_stmt(t) {
            db.set_acc_description(&acc);
            continue;
        }

        if try_parse_acc_title_as_title(t, &mut db, &meta.effective_config) {
            continue;
        }

        if let Some(v) = try_parse_acc_descr(t, &mut lines)? {
            db.set_acc_description(&v);
            continue;
        }

        if is_direction_stmt(t) {
            continue;
        }

        let Some((name, args, has_lbrace)) = parse_macro_stmt(t)? else {
            return Err(Error::DiagramParse {
                diagram_type: meta.diagram_type.clone(),
                message: format!("unsupported C4 statement: {t}"),
            });
        };

        if is_boundary_macro(&name) {
            let mut args = args;
            match name.as_str() {
                "Enterprise_Boundary" => args.insert(2, json!("ENTERPRISE")),
                "System_Boundary" => args.insert(2, json!("SYSTEM")),
                "Container_Boundary" => args.insert(2, json!("CONTAINER")),
                _ => {}
            }

            match name.as_str() {
                "Boundary" | "Enterprise_Boundary" | "System_Boundary" => {
                    db.add_person_or_system_boundary(args)?;
                }
                "Container_Boundary" => {
                    db.add_container_boundary(args)?;
                }
                "Node" | "Deployment_Node" => {
                    db.add_deployment_node("node", args)?;
                }
                "Node_L" => {
                    db.add_deployment_node("nodeL", args)?;
                }
                "Node_R" => {
                    db.add_deployment_node("nodeR", args)?;
                }
                other => {
                    return Err(Error::DiagramParse {
                        diagram_type: meta.diagram_type.clone(),
                        message: format!("unsupported boundary macro: {other}"),
                    });
                }
            }

            if !has_lbrace {
                consume_required_lbrace(&mut lines)?;
            }
            continue;
        }

        match name.as_str() {
            "Person" => db.add_person_or_system("person", &args)?,
            "Person_Ext" => db.add_person_or_system("external_person", &args)?,
            "System" => db.add_person_or_system("system", &args)?,
            "SystemDb" => db.add_person_or_system("system_db", &args)?,
            "SystemQueue" => db.add_person_or_system("system_queue", &args)?,
            "System_Ext" => db.add_person_or_system("external_system", &args)?,
            "SystemDb_Ext" => db.add_person_or_system("external_system_db", &args)?,
            "SystemQueue_Ext" => db.add_person_or_system("external_system_queue", &args)?,

            "Container" => db.add_container("container", &args)?,
            "ContainerDb" => db.add_container("container_db", &args)?,
            "ContainerQueue" => db.add_container("container_queue", &args)?,
            "Container_Ext" => db.add_container("external_container", &args)?,
            "ContainerDb_Ext" => db.add_container("external_container_db", &args)?,
            "ContainerQueue_Ext" => db.add_container("external_container_queue", &args)?,

            "Component" => db.add_component("component", &args)?,
            "ComponentDb" => db.add_component("component_db", &args)?,
            "ComponentQueue" => db.add_component("component_queue", &args)?,
            "Component_Ext" => db.add_component("external_component", &args)?,
            "ComponentDb_Ext" => db.add_component("external_component_db", &args)?,
            "ComponentQueue_Ext" => db.add_component("external_component_queue", &args)?,

            "Rel" => db.add_rel("rel", args)?,
            "BiRel" => db.add_rel("birel", args)?,
            "Rel_U" | "Rel_Up" => db.add_rel("rel_u", args)?,
            "Rel_D" | "Rel_Down" => db.add_rel("rel_d", args)?,
            "Rel_L" | "Rel_Left" => db.add_rel("rel_l", args)?,
            "Rel_R" | "Rel_Right" => db.add_rel("rel_r", args)?,
            "Rel_Back" => db.add_rel("rel_b", args)?,
            "RelIndex" => {
                let args = args.into_iter().skip(1).collect::<Vec<_>>();
                db.add_rel("rel", args)?;
            }

            "UpdateElementStyle" => db.update_el_style(args)?,
            "UpdateRelStyle" => db.update_rel_style(args)?,
            "UpdateLayoutConfig" => db.update_layout_config(args)?,

            other => {
                return Err(Error::DiagramParse {
                    diagram_type: meta.diagram_type.clone(),
                    message: format!("unsupported C4 macro: {other}"),
                });
            }
        }
    }

    Ok(db.to_model(meta))
}

fn strip_inline_comment(line: &str) -> String {
    let mut in_quotes = false;
    let mut idx = 0usize;
    let bytes = line.as_bytes();
    while idx < bytes.len() {
        let b = bytes[idx];
        if b == b'"' {
            in_quotes = !in_quotes;
            idx += 1;
            continue;
        }
        if !in_quotes && b == b'%' && idx + 1 < bytes.len() && bytes[idx + 1] == b'%' {
            return line[..idx].to_string();
        }
        idx += 1;
    }
    line.to_string()
}

fn is_direction_stmt(t: &str) -> bool {
    let mut it = t.split_whitespace();
    let Some(first) = it.next() else {
        return false;
    };
    if first != "direction" {
        return false;
    }
    matches!(it.next(), Some("TB" | "BT" | "LR" | "RL"))
}

fn next_non_empty_line<'a>(
    lines: &mut std::iter::Peekable<std::str::Lines<'a>>,
) -> Option<&'a str> {
    while let Some(l) = lines.next() {
        if !l.trim().is_empty() {
            return Some(l);
        }
    }
    None
}

fn try_parse_title(t: &str) -> Option<String> {
    if t.starts_with("title ") && t.len() >= 6 {
        return Some(t[6..].trim_end().to_string());
    }
    None
}

fn try_parse_acc_description_stmt(t: &str) -> Option<String> {
    if t.starts_with("accDescription ") && t.len() >= 15 {
        return Some(t[15..].trim_end().to_string());
    }
    None
}

fn try_parse_acc_title_as_title(t: &str, db: &mut C4Db, config: &MermaidConfig) -> bool {
    let t = t.trim_start();
    if !t.starts_with("accTitle") {
        return false;
    }
    let rest = &t["accTitle".len()..];
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return false;
    }
    let val = rest[1..].trim();
    db.set_title(val, config);
    true
}

fn try_parse_acc_descr<'a>(
    t: &str,
    lines: &mut std::iter::Peekable<std::str::Lines<'a>>,
) -> Result<Option<String>> {
    let t = t.trim_start();
    if !t.starts_with("accDescr") {
        return Ok(None);
    }

    let rest = &t["accDescr".len()..];
    let rest = rest.trim_start();
    if rest.starts_with(':') {
        let val = rest[1..].trim();
        return Ok(Some(val.to_string()));
    }

    if rest.starts_with('{') {
        let mut buf = String::new();

        // Mermaid's lexer consumes whitespace after '{' (`accDescr\s*"{"\s*`),
        // and the parser applies a single `.trim()` to the whole token.
        let mut after = rest[1..].to_string();
        if let Some(end) = after.find('}') {
            after.truncate(end);
            return Ok(Some(after.trim().to_string()));
        }
        let after = after.trim_start();
        if !after.is_empty() {
            buf.push_str(after);
        }

        while let Some(raw) = lines.next() {
            if let Some(pos) = raw.find('}') {
                let part = &raw[..pos];
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(part);
                break;
            }

            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(raw);
        }

        return Ok(Some(buf.trim().to_string()));
    }

    Ok(None)
}

fn is_boundary_macro(name: &str) -> bool {
    matches!(
        name,
        "Boundary"
            | "Enterprise_Boundary"
            | "System_Boundary"
            | "Container_Boundary"
            | "Node"
            | "Deployment_Node"
            | "Node_L"
            | "Node_R"
    )
}

fn consume_required_lbrace(lines: &mut std::iter::Peekable<std::str::Lines<'_>>) -> Result<()> {
    while let Some(peek) = lines.peek().copied() {
        if peek.trim().is_empty() {
            lines.next();
            continue;
        }
        if peek.trim() == "{" {
            lines.next();
            return Ok(());
        }
        return Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: "expected '{' after boundary".to_string(),
        });
    }
    Err(Error::DiagramParse {
        diagram_type: "c4".to_string(),
        message: "expected '{' after boundary".to_string(),
    })
}

fn parse_macro_stmt(t: &str) -> Result<Option<(String, Vec<Value>, bool)>> {
    let t = t.trim_end();
    let Some(paren) = t.find('(') else {
        return Ok(None);
    };
    let name = t[..paren].trim().to_string();
    if name.is_empty() {
        return Ok(None);
    }

    let after = &t[paren + 1..];
    let Some(end_paren) = after.rfind(')') else {
        return Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: format!("unterminated macro call: {t}"),
        });
    };

    let args_raw = &after[..end_paren];
    let rest = after[end_paren + 1..].trim();
    let mut has_lbrace = false;
    if rest.starts_with('{') {
        if rest[1..].trim().is_empty() {
            has_lbrace = true;
        } else {
            return Err(Error::DiagramParse {
                diagram_type: "c4".to_string(),
                message: format!("unexpected tokens after '{{' in macro: {t}"),
            });
        }
    } else if !rest.is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: format!("unexpected trailing tokens in macro: {t}"),
        });
    }

    let args = parse_args_csv(args_raw)?;
    Ok(Some((name, args, has_lbrace)))
}

fn parse_args_csv(input: &str) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut cur = input;
    loop {
        if cur.trim().is_empty() {
            break;
        }
        let (seg, rest) = split_next_arg(cur);
        out.push(parse_arg(seg.trim())?);
        if rest.is_none() {
            break;
        }
        cur = rest.unwrap();
    }
    Ok(out)
}

fn split_next_arg(input: &str) -> (&str, Option<&str>) {
    let mut in_quotes = false;
    for (i, c) in input.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                return (&input[..i], Some(&input[i + 1..]));
            }
            _ => {}
        }
    }
    (input, None)
}

fn parse_arg(seg: &str) -> Result<Value> {
    if seg.is_empty() {
        return Ok(json!(""));
    }

    if let Some(v) = try_parse_kv(seg)? {
        return Ok(v);
    }

    if seg.starts_with('"') {
        let s = parse_quoted(seg)?;
        return Ok(json!(s));
    }

    Ok(json!(seg.trim()))
}

fn try_parse_kv(seg: &str) -> Result<Option<Value>> {
    let seg = seg.trim_start();
    if !seg.starts_with('$') {
        return Ok(None);
    }
    let rest = &seg[1..];
    let Some(eq) = rest.find('=') else {
        return Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: format!("invalid attribute kv: {seg}"),
        });
    };
    let key = rest[..eq].trim();
    let val_raw = rest[eq + 1..].trim_start();
    if key.is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: format!("invalid attribute kv key: {seg}"),
        });
    }
    let val = parse_quoted(val_raw)?;
    let mut map = Map::new();
    map.insert(key.to_string(), json!(val));
    Ok(Some(Value::Object(map)))
}

fn parse_quoted(input: &str) -> Result<String> {
    let input = input.trim();
    let Some(rest) = input.strip_prefix('"') else {
        return Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: format!("expected quoted string, got: {input}"),
        });
    };
    let Some(end) = rest.find('"') else {
        return Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: "unterminated string".to_string(),
        });
    };
    let val = &rest[..end];
    let trailing = rest[end + 1..].trim();
    if !trailing.is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "c4".to_string(),
            message: format!("unexpected trailing tokens after string: {trailing}"),
        });
    }
    Ok(val.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseOptions};
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    #[test]
    fn c4_trailing_whitespace_after_statements_is_accepted() {
        let whitespace = " ";
        let model = parse(&format!(
            "C4Context{whitespace}\n\
title System Context diagram for Internet Banking System{whitespace}\n\
Person(customerA, \"Banking Customer A\", \"A customer of the bank, with personal bank accounts.\"){whitespace}\n"
        ));
        assert_eq!(model["c4Type"], json!("C4Context"));
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn c4_parameter_names_that_are_keywords_are_allowed() {
        let model = parse(
            r#"C4Context
title title
Person(Person, "Person", "Person")
"#,
        );
        assert_eq!(model["title"], json!("title"));
        assert_eq!(model["shapes"][0]["alias"], json!("Person"));
        assert_eq!(model["shapes"][0]["label"]["text"], json!("Person"));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("Person"));
    }

    #[test]
    fn c4_allows_default_in_parameters() {
        let model = parse(
            r#"C4Context
Person(default, "default", "default")
"#,
        );
        assert_eq!(model["shapes"][0]["alias"], json!("default"));
        assert_eq!(model["shapes"][0]["label"]["text"], json!("default"));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("default"));
    }

    #[test]
    fn c4_person_is_parsed() {
        let model = parse(
            r#"C4Context
title System Context diagram for Internet Banking System
Person(customerA, "Banking Customer A", "A customer of the bank, with personal bank accounts.")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["alias"], json!("customerA"));
        assert_eq!(
            model["shapes"][0]["label"]["text"],
            json!("Banking Customer A")
        );
        assert_eq!(
            model["shapes"][0]["descr"]["text"],
            json!("A customer of the bank, with personal bank accounts.")
        );
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("global"));
        assert_eq!(model["shapes"][0]["typeC4Shape"]["text"], json!("person"));
        assert_eq!(model["shapes"][0]["wrap"], json!(false));
    }

    #[test]
    fn c4_boundary_is_parsed() {
        let model = parse(
            r#"C4Context
title System Context diagram for Internet Banking System
Boundary(b1, "BankBoundary") {
System(SystemAA, "Internet Banking System")
}
"#,
        );

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["alias"], json!("b1"));
        assert_eq!(
            model["boundaries"][1]["label"]["text"],
            json!("BankBoundary")
        );
        assert_eq!(model["boundaries"][1]["parentBoundary"], json!("global"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("system"));

        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("b1"));
    }

    #[test]
    fn c4_person_ext_is_parsed() {
        let model = parse(
            r#"C4Context
Person_Ext(customerA, "Banking Customer A", "A customer of the bank, with personal bank accounts.")
"#,
        );
        assert_eq!(
            model["shapes"][0]["typeC4Shape"]["text"],
            json!("external_person")
        );
    }

    #[test]
    fn c4_system_variants_are_parsed() {
        let cases = [
            ("System", "system"),
            ("SystemDb", "system_db"),
            ("SystemQueue", "system_queue"),
            ("System_Ext", "external_system"),
            ("SystemDb_Ext", "external_system_db"),
            ("SystemQueue_Ext", "external_system_queue"),
        ];
        for (macro_name, kind) in cases {
            let model = parse(&format!(
                "C4Context\n\
{macro_name}(SystemAA, \"Internet Banking System\", \"Allows customers to view information about their bank accounts, and make payments.\")\n"
            ));
            assert_eq!(model["shapes"][0]["typeC4Shape"]["text"], json!(kind));
        }
    }

    #[test]
    fn c4_container_variants_are_parsed() {
        let cases = [
            ("Container", "container"),
            ("ContainerDb", "container_db"),
            ("ContainerQueue", "container_queue"),
            ("Container_Ext", "external_container"),
            ("ContainerDb_Ext", "external_container_db"),
            ("ContainerQueue_Ext", "external_container_queue"),
        ];
        for (macro_name, kind) in cases {
            let model = parse(&format!(
                "C4Context\n\
{macro_name}(ContainerAA, \"Internet Banking Container\", \"Technology\", \"Allows customers to view information about their bank accounts, and make payments.\")\n"
            ));
            assert_eq!(model["shapes"][0]["typeC4Shape"]["text"], json!(kind));
            assert_eq!(model["shapes"][0]["techn"]["text"], json!("Technology"));
        }
    }

    #[test]
    fn c4_label_can_be_kv_object() {
        let model = parse(
            r#"C4Context
Person(customerA, $sprite="users")
"#,
        );
        assert_eq!(
            model["shapes"][0]["label"]["text"]["sprite"],
            json!("users")
        );
    }

    #[test]
    fn c4_rel_is_deduped_by_from_to_like_mermaid_db() {
        let model = parse(
            r#"C4Context
Rel(a, b, "first")
Rel(a, b, "second")
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(model["rels"][0]["label"]["text"], json!("second"));
    }

    #[test]
    fn c4_relindex_ignores_index_arg() {
        let model = parse(
            r#"C4Context
RelIndex(123, a, b, "label")
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(model["rels"][0]["from"], json!("a"));
        assert_eq!(model["rels"][0]["to"], json!("b"));
        assert_eq!(model["rels"][0]["label"]["text"], json!("label"));
    }

    #[test]
    fn c4_wrap_directive_sets_wrap_true_on_nodes() {
        let model = parse(
            r#"%%{wrap}%%
C4Context
Person(a, "A", "D")
"#,
        );
        assert_eq!(model["wrap"], json!(true));
        assert_eq!(model["shapes"][0]["wrap"], json!(true));
    }

    #[test]
    fn c4_update_element_style_updates_shape_fields() {
        let model = parse(
            r#"C4Context
Person(a, "A", "D")
UpdateElementStyle(a, $bgColor="red", $borderColor="blue")
"#,
        );
        assert_eq!(model["shapes"][0]["bgColor"], json!("red"));
        assert_eq!(model["shapes"][0]["borderColor"], json!("blue"));
    }

    #[test]
    fn c4_update_element_style_can_target_boundaries() {
        let model = parse(
            r#"C4Context
Boundary(b1, "B") {
}
UpdateElementStyle(b1, $bgColor="red")
"#,
        );
        assert_eq!(model["boundaries"][1]["bgColor"], json!("red"));
    }

    #[test]
    fn c4_update_rel_style_updates_rel_fields() {
        let model = parse(
            r#"C4Context
Rel(a, b, "label")
UpdateRelStyle(a, b, $textColor="red", $lineColor="blue", $offsetX="10", $offsetY="20")
"#,
        );
        assert_eq!(model["rels"][0]["textColor"], json!("red"));
        assert_eq!(model["rels"][0]["lineColor"], json!("blue"));
        assert_eq!(model["rels"][0]["offsetX"], json!(10));
        assert_eq!(model["rels"][0]["offsetY"], json!(20));
    }

    #[test]
    fn c4_update_layout_config_enforces_minimum_one() {
        let model = parse(
            r#"C4Context
UpdateLayoutConfig(0, 0)
"#,
        );
        assert_eq!(model["layout"]["c4ShapeInRow"], json!(4));
        assert_eq!(model["layout"]["c4BoundaryInRow"], json!(2));

        let model = parse(
            r#"C4Context
UpdateLayoutConfig(3, 2)
"#,
        );
        assert_eq!(model["layout"]["c4ShapeInRow"], json!(3));
        assert_eq!(model["layout"]["c4BoundaryInRow"], json!(2));
    }

    #[test]
    fn c4_deployment_node_ignores_sprite_param_like_mermaid_db() {
        let model = parse(
            r#"C4Deployment
Node(n1, "Node", "type", "descr", $sprite="users") {
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert!(model["boundaries"][1].get("sprite").is_none());
    }

    #[test]
    fn c4_boundary_brace_can_be_on_next_line() {
        let model = parse(
            r#"C4Context
Boundary(b1, "B")
{
  Person(p1, "P")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["alias"], json!("b1"));
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("b1"));
    }

    #[test]
    fn c4_nested_boundaries_keep_parent_boundary_correct() {
        let model = parse(
            r#"C4Context
Enterprise_Boundary(ent, "Enterprise") {
  System_Boundary(sys, "System") {
    Person(p1, "P")
  }
  Person(p2, "P2")
}
"#,
        );

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(model["boundaries"][1]["alias"], json!("ent"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("ENTERPRISE"));
        assert_eq!(model["boundaries"][1]["parentBoundary"], json!("global"));

        assert_eq!(model["boundaries"][2]["alias"], json!("sys"));
        assert_eq!(model["boundaries"][2]["type"]["text"], json!("SYSTEM"));
        assert_eq!(model["boundaries"][2]["parentBoundary"], json!("ent"));

        assert_eq!(model["shapes"].as_array().unwrap().len(), 2);
        assert_eq!(model["shapes"][0]["alias"], json!("p1"));
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("sys"));
        assert_eq!(model["shapes"][1]["alias"], json!("p2"));
        assert_eq!(model["shapes"][1]["parentBoundary"], json!("ent"));
    }

    #[test]
    fn c4_container_boundary_injects_container_type() {
        let model = parse(
            r#"C4Container
Container_Boundary(cb, "CB") {
  Container(c1, "C1", "Tech", "Desc")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["alias"], json!("cb"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("CONTAINER"));
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("cb"));
    }

    #[test]
    fn c4_nested_nodes_push_and_pop_like_boundaries() {
        let model = parse(
            r#"C4Deployment
Node(n1, "N1") {
  Node_L(n2, "N2") {
    Person(p1, "P1")
  }
  Person(p2, "P2")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(model["boundaries"][1]["alias"], json!("n1"));
        assert_eq!(model["boundaries"][1]["nodeType"], json!("node"));
        assert_eq!(model["boundaries"][2]["alias"], json!("n2"));
        assert_eq!(model["boundaries"][2]["nodeType"], json!("nodeL"));
        assert_eq!(model["boundaries"][2]["parentBoundary"], json!("n1"));

        assert_eq!(model["shapes"].as_array().unwrap().len(), 2);
        assert_eq!(model["shapes"][0]["alias"], json!("p1"));
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("n2"));
        assert_eq!(model["shapes"][1]["alias"], json!("p2"));
        assert_eq!(model["shapes"][1]["parentBoundary"], json!("n1"));
    }

    #[test]
    fn c4_update_layout_config_accepts_kv_objects() {
        let model = parse(
            r#"C4Context
UpdateLayoutConfig($c4ShapeInRow="1", $c4BoundaryInRow="1")
"#,
        );
        assert_eq!(model["layout"]["c4ShapeInRow"], json!(1));
        assert_eq!(model["layout"]["c4BoundaryInRow"], json!(1));
    }

    #[test]
    fn c4_update_macros_are_noop_when_target_missing() {
        let model = parse(
            r#"C4Context
UpdateElementStyle(missing, $bgColor="red")
UpdateRelStyle(a, b, $textColor="red")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 0);
        assert_eq!(model["rels"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn c4_techn_and_descr_can_be_kv_objects() {
        let model = parse(
            r#"C4Context
Container(c1, "C1", $techn="Rust", $descr="Fast")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["techn"]["text"], json!("Rust"));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("Fast"));
    }

    #[test]
    fn c4_boundary_type_can_be_kv_object() {
        let model = parse(
            r#"C4Context
Boundary(b1, "B", $type="company") {
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("company"));
    }

    #[test]
    fn c4_empty_args_are_allowed() {
        let model = parse(
            r#"C4Context
Person(a, , "D")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["label"]["text"], json!(""));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("D"));
    }

    #[test]
    fn c4_rel_direction_macros_are_parsed() {
        let model = parse(
            r#"C4Context
Rel(a, b, "l1")
BiRel(a, b, "l2")
Rel_Up(a, b, "l3")
Rel_U(a, b, "l4")
Rel_Down(a, b, "l5")
Rel_D(a, b, "l6")
Rel_Left(a, b, "l7")
Rel_L(a, b, "l8")
Rel_Right(a, b, "l9")
Rel_R(a, b, "l10")
Rel_Back(a, b, "l11")
"#,
        );
        let rels = model["rels"].as_array().unwrap();
        assert_eq!(rels.len(), 1, "rels are deduped by (from,to)");
        assert_eq!(model["rels"][0]["from"], json!("a"));
        assert_eq!(model["rels"][0]["to"], json!("b"));
        assert_eq!(model["rels"][0]["type"], json!("rel_b"));
        assert_eq!(model["rels"][0]["label"]["text"], json!("l11"));
    }

    #[test]
    fn c4_rel_without_label_is_ignored_like_mermaid_db() {
        let model = parse(
            r#"C4Context
Rel(a, b)
Rel(a, b, )
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn c4_rel_inline_comment_is_ignored_but_not_inside_quotes() {
        let model = parse(
            r#"C4Context
Rel(a, b, "label %% not a comment") %% actual comment
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(
            model["rels"][0]["label"]["text"],
            json!("label %% not a comment")
        );
    }

    #[test]
    fn c4_label_supports_sprite_link_tags_via_kv_objects() {
        let model = parse(
            r#"C4Context
Person(p1, $sprite="users")
System(s1, $link="https://github.com/mermaidjs")
Container(c1, $tags="tag1,tag2")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 3);
        assert_eq!(
            model["shapes"][0]["label"]["text"]["sprite"],
            json!("users")
        );
        assert_eq!(
            model["shapes"][1]["label"]["text"]["link"],
            json!("https://github.com/mermaidjs")
        );
        assert_eq!(
            model["shapes"][2]["label"]["text"]["tags"],
            json!("tag1,tag2")
        );
    }

    #[test]
    fn c4_sprite_link_tags_can_be_provided_as_positional_fields() {
        let model = parse(
            r#"C4Context
Person(p1, "P", "D", $sprite="users", $tags="tag1,tag2", $link="https://example.com")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["sprite"], json!("users"));
        assert_eq!(model["shapes"][0]["tags"], json!("tag1,tag2"));
        assert_eq!(model["shapes"][0]["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_boundary_supports_sprite_link_tags_via_kv_objects_or_positional_fields() {
        let model = parse(
            r#"C4Context
Boundary(b1, $link="https://example.com") {
  Person(p1, "P1")
}
Boundary(b2, "B2", "company", $tags="tag1,tag2", $link="https://example.com") {
  Person(p2, "P2")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(
            model["boundaries"][1]["label"]["text"]["link"],
            json!("https://example.com")
        );
        assert_eq!(model["boundaries"][2]["type"]["text"], json!("company"));
        assert_eq!(model["boundaries"][2]["tags"], json!("tag1,tag2"));
        assert_eq!(model["boundaries"][2]["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_update_element_style_applies_all_supported_fields() {
        let model = parse(
            r#"C4Context
Person(p1, "P1")
Boundary(b1, "B1") {
  Person(p2, "P2")
}
UpdateElementStyle(p1, $bgColor="red", $fontColor="white", $borderColor="black", $shadowing="true", $shape="rounded", $sprite="users", $techn="Rust", $legendText="Legend", $legendSprite="book")
UpdateElementStyle(b1, $bgColor="blue")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 2);
        assert_eq!(model["shapes"][0]["bgColor"], json!("red"));
        assert_eq!(model["shapes"][0]["fontColor"], json!("white"));
        assert_eq!(model["shapes"][0]["borderColor"], json!("black"));
        assert_eq!(model["shapes"][0]["shadowing"], json!("true"));
        assert_eq!(model["shapes"][0]["shape"], json!("rounded"));
        assert_eq!(model["shapes"][0]["sprite"], json!("users"));
        assert_eq!(model["shapes"][0]["techn"], json!("Rust"));
        assert_eq!(model["shapes"][0]["legendText"], json!("Legend"));
        assert_eq!(model["shapes"][0]["legendSprite"], json!("book"));

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["bgColor"], json!("blue"));
    }

    #[test]
    fn c4_acc_title_is_mapped_to_title_like_mermaid_grammar() {
        let model = parse(
            r#"C4Context
accTitle: A11y title
"#,
        );
        assert_eq!(model["title"], json!("A11y title"));
        assert!(model["accTitle"].is_null());
    }

    #[test]
    fn c4_acc_descr_multiline_collapses_newline_whitespace_like_common_db() {
        let model = parse(
            r#"C4Context
accDescr{
first
  second
third
}
"#,
        );
        assert_eq!(model["accDescr"], json!("first\nsecond\nthird"));
    }
}
