use super::{ParseIssue, UsecaseJsonInfinity};
use crate::SourceSpan;
use serde::de::IgnoredAny;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub(super) struct ParsedJson {
    pub value: Value,
    pub property_order: BTreeMap<String, Vec<String>>,
    pub non_finite_numbers: BTreeMap<String, UsecaseJsonInfinity>,
}

/// serde_json remains authoritative for syntax. The source walk additionally
/// preserves key order and JavaScript numbers that JSON.stringify emits as null.
pub(super) fn parse(source: &str, offset: usize) -> Result<ParsedJson, ParseIssue> {
    let decoded = serde_json::from_str::<Value>(source);
    if decoded.is_err() {
        // IgnoredAny validates number syntax without requiring a finite f64.
        serde_json::from_str::<IgnoredAny>(source)
            .map_err(|error| json_error(source, offset, error))?;
    }
    let mut collector = Collector {
        source,
        offset: 0,
        depth: 0,
        property_order: BTreeMap::new(),
        non_finite_numbers: BTreeMap::new(),
        non_finite_ranges: Vec::new(),
    };
    collector.collect_value("").map_err(|local| {
        ParseIssue::new(
            "Invalid JSON token or nesting depth",
            SourceSpan {
                start: offset + local,
                end: offset + local,
            },
        )
    })?;
    let mut value = match decoded {
        Ok(value) => value,
        Err(error) if collector.non_finite_ranges.is_empty() => {
            return Err(json_error(source, offset, error));
        }
        Err(_) => {
            let mut normalized = source.to_owned();
            for range in &collector.non_finite_ranges {
                // Equal-length replacement preserves subsequent diagnostic offsets.
                let replacement = format!("null{}", " ".repeat(range.len() - 4));
                normalized.replace_range(range.clone(), &replacement);
            }
            serde_json::from_str(&normalized).map_err(|error| json_error(source, offset, error))?
        }
    };
    if !value.is_object() {
        return Err(ParseIssue::new(
            "JSON value must have an object root",
            SourceSpan {
                start: offset,
                end: offset,
            },
        ));
    }
    // JSON.parse uses binary64 for every number, including integer literals.
    normalize_json_numbers(&mut value);
    Ok(ParsedJson {
        value,
        property_order: collector.property_order,
        non_finite_numbers: collector.non_finite_numbers,
    })
}

fn json_error(source: &str, offset: usize, error: serde_json::Error) -> ParseIssue {
    let line_start = source
        .split_inclusive('\n')
        .take(error.line().saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let mut local = if error.is_eof() {
        source.len()
    } else {
        line_start
            .saturating_add(error.column().saturating_sub(1))
            .min(source.len())
    };
    while !source.is_char_boundary(local) {
        local -= 1;
    }
    ParseIssue::new(
        format!("Invalid JSON: {error}"),
        SourceSpan {
            start: offset + local,
            end: offset + local,
        },
    )
}

fn normalize_json_numbers(value: &mut Value) {
    match value {
        Value::Number(number) => {
            if let Some(number) = number.as_f64() {
                *value = crate::compatibility_json::number_value(number);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(normalize_json_numbers),
        Value::Object(values) => values.values_mut().for_each(normalize_json_numbers),
        _ => {}
    }
}

struct Collector<'a> {
    source: &'a str,
    offset: usize,
    depth: usize,
    non_finite_numbers: BTreeMap<String, UsecaseJsonInfinity>,
    non_finite_ranges: Vec<std::ops::Range<usize>>,
    property_order: BTreeMap<String, Vec<String>>,
}

impl Collector<'_> {
    fn skip_whitespace(&mut self) {
        while self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }

    fn consume(&mut self, byte: u8) -> Result<(), usize> {
        if self.source.as_bytes().get(self.offset) != Some(&byte) {
            return Err(self.offset);
        }
        self.offset += 1;
        Ok(())
    }

    fn collect_value(&mut self, pointer: &str) -> Result<(), usize> {
        self.skip_whitespace();
        match self.source.as_bytes().get(self.offset) {
            Some(byte @ (b'{' | b'[')) => {
                // The fallback validator skips values iteratively. Bound this walk
                // before serde_json's final Value decode enforces its own limit.
                if self.depth >= 128 {
                    return Err(self.offset);
                }
                let object = *byte == b'{';
                self.depth += 1;
                let result = if object {
                    self.collect_object(pointer)
                } else {
                    self.collect_array(pointer)
                };
                self.depth -= 1;
                result
            }
            Some(b'"') => self.read_string().map(|_| ()),
            Some(_) => {
                // Scalars have already been validated by serde_json.
                let start = self.offset;
                while self.source.as_bytes().get(self.offset).is_some_and(|byte| {
                    !byte.is_ascii_whitespace() && !matches!(byte, b',' | b']' | b'}')
                }) {
                    self.offset += 1;
                }
                let literal = &self.source[start..self.offset];
                if let Ok(number) = literal.parse::<f64>()
                    && number.is_infinite()
                {
                    let infinity = if number.is_sign_negative() {
                        UsecaseJsonInfinity::Negative
                    } else {
                        UsecaseJsonInfinity::Positive
                    };
                    self.non_finite_numbers.insert(pointer.to_owned(), infinity);
                    self.non_finite_ranges.push(start..self.offset);
                }
                Ok(())
            }
            None => Err(self.offset),
        }
    }

    fn collect_object(&mut self, pointer: &str) -> Result<(), usize> {
        self.consume(b'{')?;
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        self.skip_whitespace();
        if self.source.as_bytes().get(self.offset) != Some(&b'}') {
            loop {
                let property_start = self.offset;
                let property: String =
                    serde_json::from_str(self.read_string()?).map_err(|_| property_start)?;
                let property_pointer = format!(
                    "{pointer}/{}",
                    property.replace('~', "~0").replace('/', "~1")
                );
                if seen.insert(property.clone()) {
                    order.push(property);
                } else {
                    self.delete_subtree(&property_pointer);
                }
                self.skip_whitespace();
                self.consume(b':')?;
                self.collect_value(&property_pointer)?;
                self.skip_whitespace();
                if self.source.as_bytes().get(self.offset) == Some(&b'}') {
                    break;
                }
                self.consume(b',')?;
                self.skip_whitespace();
            }
        }
        self.consume(b'}')?;
        self.property_order.insert(pointer.to_owned(), order);
        Ok(())
    }

    fn collect_array(&mut self, pointer: &str) -> Result<(), usize> {
        self.consume(b'[')?;
        self.skip_whitespace();
        let mut index = 0;
        if self.source.as_bytes().get(self.offset) != Some(&b']') {
            loop {
                self.collect_value(&format!("{pointer}/{index}"))?;
                index += 1;
                self.skip_whitespace();
                if self.source.as_bytes().get(self.offset) == Some(&b']') {
                    break;
                }
                self.consume(b',')?;
            }
        }
        self.consume(b']')
    }

    fn read_string(&mut self) -> Result<&str, usize> {
        let start = self.offset;
        self.consume(b'"')?;
        while let Some(byte) = self.source.as_bytes().get(self.offset) {
            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(&self.source[start..self.offset]);
                }
                b'\\' => self.offset += 2,
                _ => self.offset += 1,
            }
        }
        Err(self.offset.min(self.source.len()))
    }

    fn delete_subtree(&mut self, pointer: &str) {
        self.property_order.remove(pointer);
        self.non_finite_numbers.remove(pointer);
        let prefix = format!("{pointer}/");
        // Only descendants are visited, so repeated scalar keys do not rescan
        // unrelated objects. The separator keeps similarly named siblings intact.
        let descendants: Vec<_> = self
            .property_order
            .range(prefix.clone()..)
            .take_while(|(key, _)| key.starts_with(&prefix))
            .map(|(key, _)| key.clone())
            .collect();
        for descendant in descendants {
            self.property_order.remove(&descendant);
        }
        let descendants: Vec<_> = self
            .non_finite_numbers
            .range(prefix.clone()..)
            .take_while(|(key, _)| key.starts_with(&prefix))
            .map(|(key, _)| key.clone())
            .collect();
        for descendant in descendants {
            self.non_finite_numbers.remove(&descendant);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_follow_javascript_binary64_and_normalize_negative_zero() {
        let ParsedJson { value, .. } = parse(r#"{"safe":9007199254740991,"rounded":9007199254740993,"nested":[-9007199254740993,1e20,1e21,-0],"boundary":9223372036854775808}"#, 0).unwrap();
        assert_eq!(value["safe"], json!(9007199254740991_i64));
        assert_eq!(value["rounded"], json!(9007199254740992_i64));
        assert_eq!(value["nested"][0], json!(-9007199254740992_i64));
        assert_eq!(value["nested"][1].as_f64(), Some(1e20));
        assert_eq!(value["nested"][2].as_f64(), Some(1e21));
        assert_eq!(value["nested"][3], json!(0));
        assert_eq!(value["boundary"].as_f64(), Some(9223372036854775808.0));
        assert_ne!(value["boundary"], json!(i64::MAX));
    }

    #[test]
    fn nested_arrays_and_escaped_json_pointers_preserve_source_order() {
        let ParsedJson { property_order: order, .. } = parse(
            r#"{"a/b":{"items":[{"~name":"first","id":1},{"id":2,"details":{"enabled":true}}]},"10":10,"2":2,"empty":{},"emptyArray":[]}"#,
            0,
        )
        .expect("valid object");
        assert_eq!(
            order,
            BTreeMap::from([
                (
                    "".into(),
                    vec![
                        "a/b".into(),
                        "10".into(),
                        "2".into(),
                        "empty".into(),
                        "emptyArray".into()
                    ]
                ),
                ("/a~1b".into(), vec!["items".into()]),
                ("/a~1b/items/0".into(), vec!["~name".into(), "id".into()]),
                ("/a~1b/items/1".into(), vec!["id".into(), "details".into()]),
                ("/a~1b/items/1/details".into(), vec!["enabled".into()]),
                ("/empty".into(), vec![]),
            ])
        );
    }

    #[test]
    fn duplicate_keys_keep_first_position_and_replace_old_subtree() {
        let ParsedJson { value, property_order: order, .. } = parse(
            r#"{"entry":{"discarded":{"old":true}},"entrySibling":{"keep":true},"tail":0,"entry":{"second":2,"first":1,"\u0073econd":3}}"#,
            0,
        )
        .expect("valid object");
        assert_eq!(
            value,
            json!({"entry":{"second":3,"first":1},"entrySibling":{"keep":true},"tail":0})
        );
        assert_eq!(
            order,
            BTreeMap::from([
                (
                    "".into(),
                    vec!["entry".into(), "entrySibling".into(), "tail".into()]
                ),
                ("/entry".into(), vec!["second".into(), "first".into()]),
                ("/entrySibling".into(), vec!["keep".into()]),
            ])
        );
        let ParsedJson {
            property_order: order,
            ..
        } = parse(r#"{"a":[{"old":{}}],"a/b":{"keep":1},"a":null}"#, 0)
            .expect("valid duplicate scalar replacement");
        assert_eq!(order.len(), 2);
        assert_eq!(order["/a~1b"], vec!["keep"]);
    }

    #[test]
    fn strings_do_not_expose_delimiters_and_invalid_json_reports_source_offset() {
        let ParsedJson {
            property_order: order,
            ..
        } = parse(
            r#"{"text":"a } { [ ] \"quoted\"","nested":{"text":"{\"key\":1}"}}"#,
            0,
        )
        .expect("valid strings");
        assert_eq!(order.len(), 2);
        assert_eq!(order["/nested"], vec!["text"]);
        let error = parse("{\r\n  \"key\": true,\r\n}", 30).expect_err("trailing comma");
        assert_eq!(error.span.start, 49);
        assert!(error.message.starts_with("Invalid JSON:"));
        let error = parse(r#"{"a":"#, 7).expect_err("incomplete value");
        assert_eq!(error.span.start, 12);
        for source in ["true", "null", "[]"] {
            let error = parse(source, 12).expect_err("root must be an object");
            assert_eq!(error.span.start, 12);
            assert_eq!(error.message, "JSON value must have an object root");
        }
    }
    #[test]
    fn finite_decimal_rounding_matches_javascript_in_core_only_builds() {
        let parsed = parse(
            r#"{"fraction":0.12345678901234567,"maximum":1.7976931348623158e308}"#,
            0,
        )
        .unwrap();
        assert_eq!(parsed.value["fraction"].as_f64(), Some(0.12345678901234566));
        assert_eq!(parsed.value["maximum"].as_f64(), Some(f64::MAX));
        assert!(parsed.non_finite_numbers.is_empty());
    }

    #[test]
    fn non_finite_numbers_preserve_render_values_and_serialize_as_null() {
        let parsed = parse(r#"{"1e309":"-1e309","a/b":[1e309,-1e309,null,-0],"nested":{"~key":1e400},"underflow":1e-9999}"#, 0).unwrap();
        assert_eq!(
            parsed.value,
            json!({"1e309":"-1e309","a/b":[null,null,null,0],"nested":{"~key":null},"underflow":0})
        );
        assert_eq!(
            parsed.non_finite_numbers,
            BTreeMap::from([
                ("/a~1b/0".into(), UsecaseJsonInfinity::Positive),
                ("/a~1b/1".into(), UsecaseJsonInfinity::Negative),
                ("/nested/~0key".into(), UsecaseJsonInfinity::Positive),
            ])
        );
    }

    #[test]
    fn duplicate_keys_clear_non_finite_values_without_losing_replacement_ranges() {
        let parsed = parse(r#"{"a":1e309,"a":null,"b":{"old":[1e309]},"b":{},"c":false,"\u0063":-1e309,"cSibling":1e309}"#, 0).unwrap();
        assert_eq!(
            parsed.value,
            json!({"a":null,"b":{},"c":null,"cSibling":null})
        );
        assert_eq!(
            parsed.non_finite_numbers,
            BTreeMap::from([
                ("/c".into(), UsecaseJsonInfinity::Negative),
                ("/cSibling".into(), UsecaseJsonInfinity::Positive),
            ])
        );
        assert!(!parsed.property_order.contains_key("/b/old"));
    }

    #[test]
    fn non_finite_fallback_preserves_json_validation_depth_and_offsets() {
        for source in [
            r#"{"x":NaN}"#,
            r#"{"x":Infinity}"#,
            r#"{"x":-Infinity}"#,
            r#"{"x":1e309,"bad":01}"#,
            r#"{"x":1e309,"bad":"\uD800"}"#,
        ] {
            assert!(parse(source, 0).is_err(), "{source}");
        }
        let source = "{\n\"x\":1e309,\n\"bad\":true,\n}";
        let error = parse(source, 31).unwrap_err();
        assert_eq!(error.span.start, 31 + source.len() - 1);
        for depth in [126, 127, 128, 300] {
            let finite = format!(r#"{{"value":{}1{}}}"#, "[".repeat(depth), "]".repeat(depth));
            let infinite = finite.replacen("1", "1e309", 1);
            assert_eq!(
                parse(&finite, 0).is_ok(),
                parse(&infinite, 0).is_ok(),
                "depth {depth}"
            );
        }
    }
}
