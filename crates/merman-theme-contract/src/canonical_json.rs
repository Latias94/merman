use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub(crate) enum CanonicalJsonError {
    #[error("canonical JSON rejected the input: {message}")]
    InvalidInput { message: String },
    #[error("canonical JSON serialization failed: {message}")]
    Serialization { message: String },
}

// Callers must first validate contract-owned wire values against the RFC 8785 I-JSON constraints.
// This stays private until those wire types exist, so arbitrary Serialize implementations cannot
// become part of the persisted theme contract.
pub(crate) fn canonical_json_bytes<T>(value: &T) -> Result<Vec<u8>, CanonicalJsonError>
where
    T: Serialize,
{
    serde_json_canonicalizer::to_vec(value).map_err(|error| {
        let message = error.to_string();
        if error.io_error_kind() == Some(std::io::ErrorKind::InvalidInput) {
            CanonicalJsonError::InvalidInput { message }
        } else {
            CanonicalJsonError::Serialization { message }
        }
    })
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde::ser::SerializeMap;

    use super::*;

    struct OrderedObject<'a>(&'a [(&'a str, i32)]);

    impl Serialize for OrderedObject<'_> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut map = serializer.serialize_map(Some(self.0.len()))?;
            for (key, value) in self.0 {
                map.serialize_entry(key, value)?;
            }
            map.end()
        }
    }

    fn canonical_string(value: &impl Serialize) -> String {
        String::from_utf8(canonical_json_bytes(value).expect("value should canonicalize"))
            .expect("JCS output must be UTF-8")
    }

    #[test]
    fn jcs_orders_object_keys_by_utf16_code_units() {
        let input = OrderedObject(&[("\u{e000}", 1), ("😀", 2)]);

        assert_eq!(canonical_string(&input), "{\"😀\":2,\"\u{e000}\":1}");
    }

    #[test]
    fn jcs_uses_ecmascript_number_serialization_and_normalizes_negative_zero() {
        let input = [
            333333333.33333329,
            1e30,
            4.50,
            2e-3,
            0.000000000000000000000000001,
            -0.0,
        ];

        assert_eq!(
            canonical_string(&input),
            "[333333333.3333333,1e+30,4.5,0.002,1e-27,0]"
        );
    }

    #[test]
    fn jcs_preserves_array_order_while_canonicalizing_nested_objects() {
        let input = serde_json::json!([3, { "z": 0, "a": 1 }, 2, 1]);

        assert_eq!(canonical_string(&input), "[3,{\"a\":1,\"z\":0},2,1]");
    }

    #[test]
    fn jcs_uses_required_control_character_escapes() {
        let input = "\u{0000}\u{0008}\t\n\u{000c}\r\"\\\u{001f}";

        assert_eq!(canonical_string(&input), r#""\u0000\b\t\n\f\r\"\\\u001f""#);
    }

    #[test]
    fn jcs_is_stable_across_object_insertion_order() {
        let first = OrderedObject(&[("beta", 2), ("alpha", 1), ("gamma", 3)]);
        let second = OrderedObject(&[("gamma", 3), ("beta", 2), ("alpha", 1)]);

        assert_eq!(canonical_json_bytes(&first), canonical_json_bytes(&second));
        assert_eq!(
            canonical_string(&first),
            "{\"alpha\":1,\"beta\":2,\"gamma\":3}"
        );
    }

    #[test]
    fn typed_canonicalization_rejects_non_finite_numbers() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                canonical_json_bytes(&value),
                Err(CanonicalJsonError::InvalidInput { .. })
            ));
        }
    }
}
