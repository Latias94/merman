use serde_json::{Number, Value};

pub(crate) fn number_value(value: f64) -> Value {
    if value.is_finite()
        && value.fract() == 0.0
        && value >= i64::MIN as f64
        && value <= i64::MAX as f64
    {
        Value::Number(Number::from(value as i64))
    } else {
        Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

pub(crate) fn string_array_value(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}
