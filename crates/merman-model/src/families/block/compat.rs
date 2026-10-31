//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{BlockClassDef, BlockDiagramRenderModel, BlockEdge, BlockNode, BlockNodeFieldPresence};
use crate::json::ManagedJson;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::warnings::legacy_warning_messages;
use crate::{OperationControl, OperationControlResult};
use indexmap::IndexMap;
use serde_json::{Map, Value, json};

pub fn project(
    model: &BlockDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    project_controlled(model, meta, &OperationControl::new())
        .expect("a private projection control cannot be cancelled")
}

pub fn project_controlled(
    model: &BlockDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, CompatProjectionError>> {
    if let Err(message) = model.validate_child_relationships() {
        return Ok(Err(CompatProjectionError::invalid_model("block", message)));
    }
    let mut blocks = Vec::<ManagedJson>::new();
    if let Some(root) = model.root() {
        for &index in &root.children {
            blocks.push(block_render_node_to_value_controlled(
                model, index, control,
            )?);
        }
    }
    let mut blocks_flat = Vec::<ManagedJson>::with_capacity(model.blocks_flat.len());
    for index in 0..model.blocks_flat.len() {
        control.checkpoint()?;
        blocks_flat.push(block_render_node_to_value_controlled(
            model, index, control,
        )?);
    }
    let edges = model
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| block_render_edge_to_value(edge, model.edge_directions(index)))
        .collect::<Vec<_>>();
    let config = ManagedJson::from(crate::json::clone_value_nonrecursive_with_control(
        meta.config.as_value(),
        control,
    )?);
    control.checkpoint()?;
    // All deep values remain managed until this checkpoint-free final handoff.
    let mut out = Map::new();
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_owned()),
    );
    out.insert(
        "blocks".to_string(),
        Value::Array(
            blocks
                .into_iter()
                .map(ManagedJson::into_unmanaged_value)
                .collect(),
        ),
    );
    out.insert("edges".to_string(), Value::Array(edges));
    out.insert(
        "blocksFlat".to_string(),
        Value::Array(
            blocks_flat
                .into_iter()
                .map(ManagedJson::into_unmanaged_value)
                .collect(),
        ),
    );
    out.insert(
        "classes".to_string(),
        block_compat_classes_to_value(&model.class_defs),
    );
    out.insert("warningFacts".to_string(), json!(&model.warning_facts));
    out.insert(
        "warnings".to_string(),
        json!(legacy_warning_messages(&model.warning_facts)),
    );
    out.insert("config".to_string(), config.into_unmanaged_value());
    Ok(Ok(Value::Object(out)))
}

fn block_render_node_to_value_shallow(
    block: &BlockNode,
    color_index: Option<usize>,
    compatibility: BlockNodeFieldPresence,
    children: Vec<Value>,
) -> Value {
    let mut obj = Map::new();
    obj.insert("id".to_string(), json!(&block.id));
    obj.insert("type".to_string(), json!(&block.block_type));
    obj.insert("label".to_string(), json!(&block.label));
    obj.insert("children".to_string(), Value::Array(children));
    if let Some(index) = color_index {
        obj.insert("colorIndex".to_string(), json!(index));
    }

    if let Some(v) = block.width {
        obj.insert("width".to_string(), json!(v));
    }
    if let Some(v) = block.columns {
        obj.insert("columns".to_string(), json!(v));
    }
    if let Some(v) = block.width_in_columns {
        obj.insert("widthInColumns".to_string(), json!(v));
    }
    if compatibility
        .directions
        .is_present(!block.directions.is_empty())
    {
        obj.insert("directions".to_string(), json!(&block.directions));
    }
    if !block.classes.is_empty() {
        obj.insert("classes".to_string(), json!(&block.classes));
    }
    if compatibility.styles.is_present(!block.styles.is_empty()) {
        obj.insert("styles".to_string(), json!(&block.styles));
    }

    Value::Object(obj)
}

fn block_render_node_to_value_controlled(
    model: &BlockDiagramRenderModel,
    index: usize,
    control: &OperationControl,
) -> OperationControlResult<ManagedJson> {
    struct Frame {
        index: usize,
        next: usize,
        children: Vec<ManagedJson>,
    }
    let mut stack = vec![Frame {
        index,
        next: 0,
        children: Vec::new(),
    }];
    let mut steps = 0usize;
    loop {
        if steps.is_multiple_of(128) {
            control.checkpoint()?;
        }
        steps += 1;
        let Some(frame) = stack.last_mut() else {
            return Ok(Value::Null.into());
        };
        let block = &model.blocks_flat[frame.index];
        if let Some(&child) = block.children.get(frame.next) {
            frame.next += 1;
            stack.push(Frame {
                index: child,
                next: 0,
                children: Vec::new(),
            });
            continue;
        }
        let Some(frame) = stack.pop() else {
            return Ok(Value::Null.into());
        };
        // Raw values exist only during this checkpoint-free assembly, then regain managed ownership.
        let children = frame
            .children
            .into_iter()
            .map(ManagedJson::into_unmanaged_value)
            .collect();
        let color_index = model
            .visuals
            .color_indices
            .get(frame.index)
            .copied()
            .flatten();
        let compatibility = model.node_field_presence(frame.index);
        let value = block_render_node_to_value_shallow(block, color_index, compatibility, children);
        let value = ManagedJson::from(value);
        if let Some(parent) = stack.last_mut() {
            parent.children.push(value);
        } else {
            return Ok(value);
        }
    }
}

fn block_render_edge_to_value(edge: &BlockEdge, directions: Option<&[String]>) -> Value {
    let mut obj = Map::new();
    obj.insert("id".to_string(), json!(&edge.id));
    obj.insert("type".to_string(), json!("edge"));
    obj.insert("label".to_string(), json!(&edge.label));
    obj.insert("children".to_string(), json!([]));
    obj.insert("start".to_string(), json!(&edge.start));
    obj.insert("end".to_string(), json!(&edge.end));
    if let Some(value) = &edge.arrow_type_end {
        obj.insert("arrowTypeEnd".to_string(), json!(value));
    }
    if let Some(value) = &edge.arrow_type_start {
        obj.insert("arrowTypeStart".to_string(), json!(value));
    }
    if let Some(directions) = directions {
        obj.insert("directions".to_string(), json!(directions));
    }
    Value::Object(obj)
}

fn block_compat_classes_to_value(classes: &IndexMap<String, BlockClassDef>) -> Value {
    let mut obj = Map::new();
    for (k, v) in classes {
        obj.insert(
            k.clone(),
            json!({
                "id": v.id,
                "styles": v.styles,
                "textStyles": v.text_styles,
            }),
        );
    }
    Value::Object(obj)
}
