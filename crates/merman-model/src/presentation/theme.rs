//! Bounded numeric values shared by theme constructors and output consumers.
//!
//! This module performs no theme evaluation or source configuration admission.

use serde_json::Value;

/// Maximum generated color slots accepted by the pinned Mermaid theme programs.
pub const MAX_THEME_COLOR_ITERATIONS: usize = 64;

/// A bounded Mermaid theme program requested more generated color slots than Merman admits.
///
/// Mermaid exposes `themeVariables.THEME_COLOR_LIMIT` as mutable input and uses it as the bound
/// for several constructor loops. Merman preserves that behavior within a fixed work ceiling so
/// hostile or accidental values cannot turn configuration materialization into unbounded work.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Mermaid theme evaluation limit `{limit}` exceeded: requested={requested}, max={max}")]
pub struct ThemeEvaluationLimitExceeded {
    pub limit: &'static str,
    pub requested: String,
    pub max: usize,
}

/// Converts the JSON-reachable `THEME_COLOR_LIMIT` value to a bounded loop count.
///
/// Positive finite values round upward, while absent, non-numeric, zero, and negative values
/// produce no iterations. The scalar and singleton-array coercions preserve the existing
/// Mermaid constructor behavior without implementing general JavaScript coercion.
pub fn theme_color_iterations(raw: Option<&Value>) -> Result<usize, ThemeEvaluationLimitExceeded> {
    // Mermaid declares this field as a number. Preserve the scalar coercions reached through raw
    // YAML/JSON compatibility input, but keep the seam deliberately narrower than JavaScript's
    // general ToNumber algorithm.
    let limit = raw.map_or(f64::NAN, |value| match value {
        Value::Null | Value::Bool(false) => 0.0,
        Value::Bool(true) => 1.0,
        Value::Number(value) => value.as_f64().unwrap_or(f64::NAN),
        Value::String(value) => parse_theme_color_limit_string(value),
        Value::Array(value) => parse_theme_color_limit_array(value),
        Value::Object(_) => f64::NAN,
    });
    if limit.is_nan() || limit <= 0.0 {
        return Ok(0);
    }

    let requested_iterations = limit.ceil();
    if !requested_iterations.is_finite() || requested_iterations > MAX_THEME_COLOR_ITERATIONS as f64
    {
        return Err(ThemeEvaluationLimitExceeded {
            limit: "THEME_COLOR_LIMIT",
            requested: raw.map_or_else(|| "missing".to_string(), requested_value_json),
            max: MAX_THEME_COLOR_ITERATIONS,
        });
    }
    Ok(requested_iterations as usize)
}

fn requested_value_json(value: &Value) -> String {
    // This error can also be reached through arbitrarily deep singleton-array coercion.
    // Keep diagnostic export on the same iterative lifecycle as other shared JSON values.
    let value = crate::ManagedJson::from(value);
    let mut bytes = Vec::new();
    value
        .write_json(&mut bytes)
        .expect("writing an existing JSON value into a byte vector cannot fail");
    String::from_utf8(bytes).expect("JSON serialization produces UTF-8")
}

fn parse_theme_color_limit_string(value: &str) -> f64 {
    let value = value.trim_matches(crate::text::is_ecmascript_whitespace);
    if value.is_empty() {
        return 0.0;
    }
    match value {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let unsigned = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    if unsigned.eq_ignore_ascii_case("inf") || unsigned.eq_ignore_ascii_case("infinity") {
        return f64::NAN;
    }
    for (prefixes, radix) in [(["0x", "0X"], 16), (["0b", "0B"], 2), (["0o", "0O"], 8)] {
        if let Some(digits) = prefixes
            .iter()
            .find_map(|prefix| value.strip_prefix(prefix))
        {
            if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
                return f64::NAN;
            }
            return match usize::from_str_radix(digits, radix) {
                Ok(parsed) if parsed <= MAX_THEME_COLOR_ITERATIONS => parsed as f64,
                Ok(_) | Err(_) => (MAX_THEME_COLOR_ITERATIONS + 1) as f64,
            };
        }
    }
    value.parse::<f64>().unwrap_or(f64::NAN)
}

fn parse_theme_color_limit_array(mut value: &[Value]) -> f64 {
    loop {
        let [only] = value else {
            return if value.is_empty() { 0.0 } else { f64::NAN };
        };
        match only {
            Value::Null => return 0.0,
            Value::Bool(_) | Value::Object(_) => return f64::NAN,
            Value::Number(value) => return value.as_f64().unwrap_or(f64::NAN),
            Value::String(value) => return parse_theme_color_limit_string(value),
            Value::Array(nested) => value = nested,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn iteration_count(value: Value) -> Result<usize, ThemeEvaluationLimitExceeded> {
        theme_color_iterations(Some(&value))
    }

    #[test]
    fn deep_over_limit_array_diagnostics_export_without_recursion() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut value = json!("Infinity");
                for _ in 0..4096 {
                    value = Value::Array(vec![value]);
                }
                let value = crate::ManagedJson::from_value(value);
                let error = theme_color_iterations(Some(value.as_value())).unwrap_err();
                assert_eq!(error.requested.len(), 4096 * 2 + "\"Infinity\"".len());
                assert!(error.requested.starts_with("[[[["));
                assert!(error.requested.ends_with("]]]]"));
                assert!(error.requested.contains("\"Infinity\""));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn theme_color_limit_uses_the_json_reachable_javascript_number_subset() {
        for value in [
            json!("0x410z"),
            json!("inf"),
            json!("infinity"),
            json!([true]),
            json!([1, 2]),
            json!({ "value": 2 }),
        ] {
            assert_eq!(iteration_count(value).unwrap(), 0);
        }
        for value in [json!("\u{feff}2\u{feff}"), json!([2]), json!([["2"]])] {
            assert_eq!(iteration_count(value).unwrap(), 2);
        }
        for value in [json!([]), json!([null]), json!([[[]]])] {
            assert_eq!(iteration_count(value).unwrap(), 0);
        }
        for value in [json!(65), json!("0x41"), json!("Infinity"), json!([65])] {
            assert_eq!(iteration_count(value).unwrap_err().max, 64);
        }
    }

    #[test]
    fn iteration_ceiling_preserves_fractional_bounds_and_error_fields() {
        assert_eq!(theme_color_iterations(None).unwrap(), 0);
        for (value, expected) in [
            (json!(true), 1),
            (json!(false), 0),
            (json!(-1), 0),
            (json!(0.25), 1),
            (json!(63.25), 64),
            (json!(64), 64),
        ] {
            assert_eq!(iteration_count(value).unwrap(), expected);
        }
        let error = iteration_count(json!(64.25)).unwrap_err();
        assert_eq!(error.limit, "THEME_COLOR_LIMIT");
        assert_eq!(error.requested, "64.25");
        assert_eq!(error.max, 64);
        assert_eq!(
            error.to_string(),
            "Mermaid theme evaluation limit `THEME_COLOR_LIMIT` exceeded: requested=64.25, max=64"
        );
    }
}
