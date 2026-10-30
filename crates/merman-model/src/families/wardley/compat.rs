//! Pure Mermaid-compatible JSON projection of declared family data.

use super::WardleyDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::{OperationControl, OperationControlResult};
use serde::Serialize;
use serde_json::{Map, Value, json};

pub fn project(
    model: &WardleyDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let control = OperationControl::new();
    project_controlled(model, meta, &control).expect("a private parse control cannot be cancelled")
}

pub fn project_controlled(
    model: &WardleyDiagramModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, CompatProjectionError>> {
    control.checkpoint()?;
    let nodes = serialize_wardley_items_controlled(&model.nodes, control)?;
    let links = serialize_wardley_items_controlled(&model.links, control)?;
    let trends = serialize_wardley_items_controlled(&model.trends, control)?;
    let annotations = serialize_wardley_items_controlled(&model.annotations, control)?;
    let notes = serialize_wardley_items_controlled(&model.notes, control)?;
    let accelerators = serialize_wardley_items_controlled(&model.accelerators, control)?;
    let deaccelerators = serialize_wardley_items_controlled(&model.deaccelerators, control)?;

    let mut pipelines = Vec::with_capacity(model.pipelines.len());
    for (pipeline_index, pipeline) in model.pipelines.iter().enumerate() {
        if pipeline_index % 128 == 0 {
            control.checkpoint()?;
        }
        let mut component_ids = Vec::with_capacity(pipeline.component_ids.len());
        for (component_index, component_id) in pipeline.component_ids.iter().enumerate() {
            if component_index % 128 == 0 {
                control.checkpoint()?;
            }
            component_ids.push(Value::String(component_id.clone()));
        }
        pipelines.push(json!({
            "nodeId": pipeline.node_id,
            "componentIds": component_ids,
        }));
    }

    let mut axes = Map::new();
    if let Some(x_label) = &model.axes.x_label {
        axes.insert("xLabel".to_string(), Value::String(x_label.clone()));
    }
    if let Some(y_label) = &model.axes.y_label {
        axes.insert("yLabel".to_string(), Value::String(y_label.clone()));
    }
    if !model.axes.stages.is_empty() {
        axes.insert(
            "stages".to_string(),
            Value::Array(strings_to_json_controlled(&model.axes.stages, control)?),
        );
    }
    if !model.axes.stage_boundaries.is_empty() {
        let mut boundaries = Vec::with_capacity(model.axes.stage_boundaries.len());
        for (index, boundary) in model.axes.stage_boundaries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            boundaries.push(json!(boundary));
        }
        axes.insert("stageBoundaries".to_string(), Value::Array(boundaries));
    }

    let mut out = Map::with_capacity(14);
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_owned()),
    );
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("nodes".to_string(), Value::Array(nodes));
    out.insert("links".to_string(), Value::Array(links));
    out.insert("trends".to_string(), Value::Array(trends));
    out.insert("pipelines".to_string(), Value::Array(pipelines));
    out.insert("annotations".to_string(), Value::Array(annotations));
    out.insert("notes".to_string(), Value::Array(notes));
    out.insert("accelerators".to_string(), Value::Array(accelerators));
    out.insert("deaccelerators".to_string(), Value::Array(deaccelerators));
    out.insert("annotationsBox".to_string(), json!(&model.annotations_box));
    out.insert("axes".to_string(), Value::Object(axes));
    out.insert("size".to_string(), json!(&model.size));
    control.checkpoint()?;
    Ok(Ok(Value::Object(out)))
}

fn serialize_wardley_items_controlled<T: Serialize>(
    items: &[T],
    control: &OperationControl,
) -> OperationControlResult<Vec<Value>> {
    let mut values = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        values.push(json!(item));
    }
    Ok(values)
}

fn strings_to_json_controlled(
    strings: &[String],
    control: &OperationControl,
) -> OperationControlResult<Vec<Value>> {
    let mut values = Vec::with_capacity(strings.len());
    for (index, value) in strings.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        values.push(Value::String(value.clone()));
    }
    Ok(values)
}
