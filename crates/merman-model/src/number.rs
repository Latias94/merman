//! Numeric value conversion shared by source and output consumers.

/// Canonical Serde representation of a JavaScript Number, including nonfinite states.
///
/// Finite whole values retain their integer JSON shape. Nonfinite values use exactly `"Infinity"`,
/// `"-Infinity"`, or `"NaN"`; Mermaid compatibility output instead uses `json::number_value`.
pub mod serde_number {
    use serde::{Deserialize, Serialize};

    pub fn serialize<S: serde::Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        if value.is_nan() {
            serializer.serialize_str("NaN")
        } else if *value == f64::INFINITY {
            serializer.serialize_str("Infinity")
        } else if *value == f64::NEG_INFINITY {
            serializer.serialize_str("-Infinity")
        } else if *value == 0.0 && value.is_sign_negative() {
            serializer.serialize_f64(*value)
        } else {
            crate::json::number_value(*value).serialize(serializer)
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Number {
            Finite(f64),
            Nonfinite(String),
        }
        match Number::deserialize(deserializer)? {
            Number::Finite(value) => Ok(value),
            Number::Nonfinite(value) => match value.as_str() {
                "Infinity" => Ok(f64::INFINITY),
                "-Infinity" => Ok(f64::NEG_INFINITY),
                "NaN" => Ok(f64::NAN),
                _ => Err(serde::de::Error::custom(
                    "expected a number, \"Infinity\", \"-Infinity\", or \"NaN\"",
                )),
            },
        }
    }
}

/// Optional canonical Number; `null` denotes absence rather than an erased nonfinite value.
pub mod serde_optional_number {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    struct Number(#[serde(with = "super::serde_number")] f64);

    pub fn serialize<S: serde::Serializer>(
        value: &Option<f64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(Number).serialize(serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<f64>, D::Error> {
        Ok(Option::<Number>::deserialize(deserializer)?.map(|value| value.0))
    }
}

/// A canonical Number list with the same nonfinite states as scalar Numbers.
pub mod serde_number_list {
    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    struct Number(#[serde(with = "super::serde_number")] f64);

    pub fn serialize<S: serde::Serializer>(
        values: &[f64],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(values.len()))?;
        for value in values {
            sequence.serialize_element(&Number(*value))?;
        }
        sequence.end()
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<f64>, D::Error> {
        Ok(Vec::<Number>::deserialize(deserializer)?
            .into_iter()
            .map(|number| number.0)
            .collect())
    }
}

/// Converts unsigned base-2, base-8, or base-16 digits with one binary64 rounding step.
///
/// The caller owns prefixes, signs, and token boundaries. Empty or invalid digits are rejected;
/// values exceeding binary64 range become positive infinity.
pub fn parse_power_of_two_digits(digits: &str, radix: u32) -> Option<f64> {
    if !matches!(radix, 2 | 8 | 16)
        || digits.is_empty()
        || !digits.chars().all(|digit| digit.is_digit(radix))
    {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(0.0);
    }
    let bits_per_digit = radix.ilog2() as usize;
    let leading_len = digits.len().min(64 / bits_per_digit);
    let leading = u64::from_str_radix(&digits[..leading_len], radix).ok()?;
    let trailing = &digits[leading_len..];
    // A sticky bit retains whether discarded digits cross a rounding midpoint.
    let sticky = u64::from(trailing.bytes().any(|digit| digit != b'0'));
    let shift = trailing.len().saturating_mul(bits_per_digit);
    Some(if shift >= 1024 {
        f64::INFINITY
    } else {
        ((leading | sticky) as f64) * 2_f64.powi(shift as i32)
    })
}
