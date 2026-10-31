//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{GanttDiagramRenderModel, GanttOutputData, GanttRenderTask};
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

pub fn project(
    model: &GanttDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let control = OperationControl::new();
    project_controlled(model, meta, &control).expect("a private parse control cannot be cancelled")
}

pub fn project_controlled(
    model: &GanttDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, CompatProjectionError>> {
    control.checkpoint()?;
    if model.output_data() == GanttOutputData::Empty {
        return Ok(Ok(json!({})));
    }

    let includes = strings_to_json_controlled(&model.includes, control)?;
    let excludes = strings_to_json_controlled(&model.excludes, control)?;
    let sections = strings_to_json_controlled(&model.sections, control)?;
    let tasks = serialize_gantt_tasks_controlled(&model.tasks, control)?;
    let links = string_map_to_json_controlled(&model.links, control)?;
    let click_events = serialize_map_to_json_controlled(&model.click_events, control)?;

    let mut out = Map::with_capacity(19);
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_owned()),
    );
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert(
        "dateFormat".to_string(),
        Value::String(model.date_format.clone()),
    );
    out.insert(
        "axisFormat".to_string(),
        Value::String(model.axis_format.clone()),
    );
    out.insert("tickInterval".to_string(), json!(&model.tick_interval));
    out.insert(
        "todayMarker".to_string(),
        Value::String(model.today_marker.clone()),
    );
    out.insert("includes".to_string(), Value::Array(includes));
    out.insert("excludes".to_string(), Value::Array(excludes));
    out.insert(
        "inclusiveEndDates".to_string(),
        Value::Bool(model.inclusive_end_dates),
    );
    out.insert("topAxis".to_string(), Value::Bool(model.top_axis));
    out.insert("weekday".to_string(), Value::String(model.weekday.clone()));
    out.insert("weekend".to_string(), Value::String(model.weekend.clone()));
    out.insert(
        "displayMode".to_string(),
        Value::String(model.display_mode.clone()),
    );
    out.insert("sections".to_string(), Value::Array(sections));
    out.insert("tasks".to_string(), Value::Array(tasks));
    out.insert("links".to_string(), Value::Object(links));
    out.insert("clickEvents".to_string(), Value::Object(click_events));
    control.checkpoint()?;
    Ok(Ok(Value::Object(out)))
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

fn serialize_gantt_tasks_controlled(
    tasks: &[GanttRenderTask],
    control: &OperationControl,
) -> OperationControlResult<Vec<Value>> {
    let mut values = Vec::with_capacity(tasks.len());
    for (index, task) in tasks.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let mut value = json!(task);
        if let Some(object) = value.as_object_mut() {
            object.remove("sectionIndex");
            object.remove("startConstraint");
            object.remove("endConstraint");
        }
        values.push(value);
    }
    Ok(values)
}

fn string_map_to_json_controlled(
    values: &HashMap<String, String>,
    control: &OperationControl,
) -> OperationControlResult<Map<String, Value>> {
    let mut out = Map::with_capacity(values.len());
    for (index, (key, value)) in values.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        out.insert(key.clone(), Value::String(value.clone()));
    }
    Ok(out)
}

fn serialize_map_to_json_controlled<T: serde::Serialize>(
    values: &HashMap<String, T>,
    control: &OperationControl,
) -> OperationControlResult<Map<String, Value>> {
    let mut out = Map::with_capacity(values.len());
    for (index, (key, value)) in values.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        out.insert(key.clone(), json!(value));
    }
    Ok(out)
}
