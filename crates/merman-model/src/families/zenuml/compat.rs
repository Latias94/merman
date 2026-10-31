//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{ZenumlDiagramModel, model_to_typed_json_controlled};
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::{OperationControl, OperationControlResult};
use serde_json::{Value, json};

pub fn project(
    model: &ZenumlDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    project_controlled(model, meta, &OperationControl::new())
        .expect("a private projection control cannot be cancelled")
}

pub fn project_controlled(
    model: &ZenumlDiagramModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, CompatProjectionError>> {
    let mut projected = match model_to_typed_json_controlled(model, control)? {
        Ok(projected) => projected,
        Err(error) => {
            return Ok(Err(CompatProjectionError::invalid_model(
                meta.diagram_type.to_owned(),
                error.to_string(),
            )));
        }
    };
    for (index, participant) in projected.as_value_mut()["participants"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let object = participant.as_object_mut().unwrap();
        let width = object
            .remove("widthSource")
            .and_then(|width| width.as_str().and_then(project_js_integer))
            .and_then(serde_json::Number::from_f64)
            .map_or(Value::Null, Value::Number);
        object.insert("width".into(), width);
    }
    projected
        .as_value_mut()
        .as_object_mut()
        .unwrap()
        .insert("type".into(), json!(meta.diagram_type));
    control.checkpoint()?;
    Ok(Ok(projected.into_unmanaged_value()))
}

fn project_js_integer(source: &str) -> Option<f64> {
    let parsed = source.parse::<f64>().ok()?;
    (parsed != 0.0).then_some(parsed)
}
