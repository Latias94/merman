use serde_json::{Number, Value};

pub(crate) fn number_value(value: f64) -> Value {
    if value.is_finite()
        && value.fract() == 0.0
        && value >= i64::MIN as f64
        && value < i64::MAX as f64
    {
        Value::Number(Number::from(value as i64))
    } else {
        Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

#[cfg(feature = "diagram-state")]
pub(crate) fn string_array_value(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_projection_does_not_saturate_at_the_rounded_i64_upper_bound() {
        let upper_bound = i64::MAX as f64;
        let projected = number_value(upper_bound);
        assert_eq!(projected.as_f64(), Some(upper_bound));
        assert_ne!(projected, Value::Number(Number::from(i64::MAX)));
        assert_eq!(
            number_value(i64::MIN as f64),
            Value::Number(Number::from(i64::MIN))
        );
        assert_eq!(number_value(-0.0), Value::Number(Number::from(0)));
        assert_eq!(number_value(f64::INFINITY), Value::Null);
    }
}
