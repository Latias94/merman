//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{XyChartAxisModel, XyChartDiagramRenderModel, XyChartPlotModel, XyChartPlotType};
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Map, Number, Value};

pub fn project(
    model: &XyChartDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut out = Map::with_capacity(10);
    out.insert(
        "orientation".to_string(),
        Value::String(model.orientation.clone()),
    );
    out.insert("title".to_string(), option_string_value(&model.title));
    out.insert(
        "accTitle".to_string(),
        option_string_value(&model.acc_title),
    );
    out.insert(
        "accDescr".to_string(),
        option_string_value(&model.acc_descr),
    );
    out.insert("xAxis".to_string(), axis_value(&model.x_axis));
    out.insert("yAxis".to_string(), axis_value(&model.y_axis));
    out.insert("plots".to_string(), plots_value(&model.plots));
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(meta.config.as_value()),
    );
    Ok(Value::Object(out))
}

fn option_string_value(value: &Option<String>) -> Value {
    value
        .as_ref()
        .map(|value| Value::String(value.clone()))
        .unwrap_or(Value::Null)
}

fn optional_f64_value(value: Option<f64>) -> Value {
    value
        .and_then(Number::from_f64)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn f64_value(value: f64) -> Value {
    Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn string_array_value(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

fn axis_value(axis: &XyChartAxisModel) -> Value {
    let mut out = Map::new();
    match axis {
        XyChartAxisModel::Band { title, categories } => {
            out.insert("type".to_string(), Value::String("band".to_string()));
            out.insert("title".to_string(), Value::String(title.clone()));
            out.insert("categories".to_string(), string_array_value(categories));
        }
        XyChartAxisModel::Linear { title, min, max } => {
            out.insert("type".to_string(), Value::String("linear".to_string()));
            out.insert("title".to_string(), Value::String(title.clone()));
            out.insert("min".to_string(), optional_f64_value(*min));
            out.insert("max".to_string(), optional_f64_value(*max));
        }
    }
    Value::Object(out)
}

fn plots_value(plots: &[XyChartPlotModel]) -> Value {
    Value::Array(plots.iter().map(plot_value).collect())
}

fn plot_value(plot: &XyChartPlotModel) -> Value {
    let mut out = Map::new();
    out.insert(
        "type".to_string(),
        Value::String(plot_type_name(plot.plot_type)),
    );
    out.insert(
        "title".to_string(),
        Value::String(plot.title.clone().unwrap_or_default()),
    );
    out.insert(
        "values".to_string(),
        Value::Array(plot.values.iter().copied().map(f64_value).collect()),
    );
    out.insert("data".to_string(), plot_data_value(&plot.data));
    if !plot.point_labels.is_empty() {
        out.insert(
            "pointLabels".to_string(),
            Value::Array(
                plot.point_labels
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    Value::Object(out)
}

fn plot_type_name(plot_type: XyChartPlotType) -> String {
    match plot_type {
        XyChartPlotType::Line => "line".to_string(),
        XyChartPlotType::Bar => "bar".to_string(),
    }
}

fn plot_data_value(data: &[(String, Option<f64>)]) -> Value {
    Value::Array(
        data.iter()
            .map(|(category, value)| {
                Value::Array(vec![
                    Value::String(category.clone()),
                    optional_f64_value(*value),
                ])
            })
            .collect(),
    )
}
