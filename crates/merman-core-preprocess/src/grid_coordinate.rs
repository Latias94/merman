//! Shared coordinate admission from Mermaid 12.4.0 `gridPlacement.ts`.

use serde_json::{Number, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridCoordinateField {
    Row,
    Column,
}

impl GridCoordinateField {
    pub const fn from_key(key: &str) -> Option<Self> {
        match key.as_bytes() {
            b"row" => Some(Self::Row),
            b"column" => Some(Self::Column),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Column => "column",
        }
    }
}

/// The JavaScript type and value needed for a coordinate diagnostic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridCoordinateValue<'a> {
    Number(f64),
    String(&'a str),
    Boolean(bool),
    Null,
    Array,
    Object,
}

impl<'a> From<&'a Value> for GridCoordinateValue<'a> {
    fn from(value: &'a Value) -> Self {
        match value {
            Value::Number(number) => Self::Number(
                number
                    .as_f64()
                    .unwrap_or_else(|| number.to_string().parse().unwrap_or(f64::NAN)),
            ),
            Value::String(value) => Self::String(value),
            Value::Bool(value) => Self::Boolean(*value),
            Value::Null => Self::Null,
            Value::Array(_) => Self::Array,
            Value::Object(_) => Self::Object,
        }
    }
}

#[derive(Debug)]
pub struct GridCoordinateError<'a> {
    pub placement_id: &'a str,
    pub occurrence_index: Option<usize>,
    pub field: GridCoordinateField,
    pub received: GridCoordinateValue<'a>,
}

impl GridCoordinateError<'_> {
    pub const CODE: &'static str = "GRID_INVALID_COORDINATE";
}

impl fmt::Display for GridCoordinateError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: Invalid {} for placement \"{}",
            Self::CODE,
            self.field.as_str(),
            self.placement_id
        )?;
        if let Some(index) = self.occurrence_index {
            write!(f, "[{index}]")?;
        }
        f.write_str("\": expected a safe integer; received ")?;
        match self.received {
            GridCoordinateValue::Number(value) => {
                write_js_number(f, value)?;
                f.write_str(" (number).")
            }
            GridCoordinateValue::String(value) => {
                let quoted = serde_json::to_string(value).map_err(|_| fmt::Error)?;
                write!(f, "{quoted} (string).")
            }
            GridCoordinateValue::Boolean(value) => write!(f, "{value} (boolean)."),
            GridCoordinateValue::Null => f.write_str("null (null)."),
            GridCoordinateValue::Array => f.write_str("[array] (object)."),
            GridCoordinateValue::Object => f.write_str("[object] (object)."),
        }
    }
}

impl std::error::Error for GridCoordinateError<'_> {}

/// `None` is an omitted field. An authored null is a value and fails validation.
pub fn validate_grid_coordinate<'a>(
    placement_id: &'a str,
    occurrence_index: Option<usize>,
    field: GridCoordinateField,
    value: Option<GridCoordinateValue<'a>>,
) -> Result<(), GridCoordinateError<'a>> {
    const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

    let Some(received) = value else {
        return Ok(());
    };
    // Compare the decoded binary64 Number, including both signs of zero.
    if matches!(received, GridCoordinateValue::Number(value)
        if value.abs() <= MAX_SAFE_INTEGER && value.fract() == 0.0)
    {
        return Ok(());
    }
    Err(GridCoordinateError {
        placement_id,
        occurrence_index,
        field,
        received,
    })
}

fn write_js_number(f: &mut fmt::Formatter<'_>, value: f64) -> fmt::Result {
    let Some(number) = Number::from_f64(value) else {
        return f.write_str(if value.is_nan() {
            "NaN"
        } else if value.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        });
    };
    // Reuse serde_json's shortest binary64 digits and apply Number's decimal notation range.
    let text = number.to_string();
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return f.write_str(text.strip_suffix(".0").unwrap_or(&text));
    };
    let exponent = exponent.parse::<i32>().map_err(|_| fmt::Error)?;
    let mantissa = mantissa.strip_suffix(".0").unwrap_or(mantissa);
    if !(-6..=20).contains(&exponent) {
        return write!(f, "{mantissa}e{exponent:+}");
    }
    let mantissa = if let Some(unsigned) = mantissa.strip_prefix('-') {
        f.write_str("-")?;
        unsigned
    } else {
        mantissa
    };
    let digits = mantissa.replace('.', "");
    let point = exponent + 1;
    if point <= 0 {
        return write!(f, "0.{:0>width$}{digits}", "", width = (-point) as usize);
    }
    let point = point as usize;
    if point >= digits.len() {
        return write!(f, "{digits:0<point$}");
    }
    let (whole, fraction) = digits.split_at(point);
    write!(f, "{whole}.{fraction}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_omission_and_signed_safe_numbers() {
        let row = |value| validate_grid_coordinate("A", None, GridCoordinateField::Row, value);
        assert!(row(None).is_ok());
        for number in [
            -9_007_199_254_740_991.0,
            -1.0,
            -0.0,
            0.0,
            1.0,
            9_007_199_254_740_991.0,
        ] {
            assert!(
                row(Some(GridCoordinateValue::Number(number))).is_ok(),
                "{number}"
            );
        }
    }

    #[test]
    fn invalid_coordinates_report_javascript_values_and_types() {
        use GridCoordinateValue::{Array, Boolean, Null, Number, Object, String};

        let large_json_integer = Value::from(u64::MAX);
        for (value, description) in [
            (Number(-1.5), "-1.5 (number)"),
            (Number(1.5), "1.5 (number)"),
            (
                Number(-9_007_199_254_740_992.0),
                "-9007199254740992 (number)",
            ),
            (Number(9_007_199_254_740_992.0), "9007199254740992 (number)"),
            (
                (&large_json_integer).into(),
                "18446744073709552000 (number)",
            ),
            (Number(1e-6), "0.000001 (number)"),
            (Number(-1e-7), "-1e-7 (number)"),
            (Number(1e20), "100000000000000000000 (number)"),
            (Number(1e21), "1e+21 (number)"),
            (Number(f64::NAN), "NaN (number)"),
            (Number(f64::INFINITY), "Infinity (number)"),
            (Number(f64::NEG_INFINITY), "-Infinity (number)"),
            (String("1"), r#""1" (string)"#),
            (String("line\n\"\\"), r#""line\n\"\\" (string)"#),
            (Null, "null (null)"),
            (Boolean(false), "false (boolean)"),
            (Array, "[array] (object)"),
            (Object, "[object] (object)"),
        ] {
            let error = validate_grid_coordinate(
                "service",
                Some(1),
                GridCoordinateField::Column,
                Some(value),
            )
            .unwrap_err();
            assert_eq!(error.placement_id, "service");
            assert_eq!(error.occurrence_index, Some(1));
            assert_eq!(error.field, GridCoordinateField::Column);
            assert_eq!(
                error.to_string(),
                format!(
                    "GRID_INVALID_COORDINATE: Invalid column for placement \"service[1]\": expected a safe integer; received {description}."
                )
            );
        }
    }
}
