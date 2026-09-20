use super::ParseIssue;
use crate::SourceSpan;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// JSON decoding remains authoritative; the subsequent walk records source key
/// order, which a decoded object cannot recover for duplicate or numeric keys.
pub(super) fn parse(
    source: &str,
    offset: usize,
) -> Result<(Value, BTreeMap<String, Vec<String>>), ParseIssue> {
    let value: Value = serde_json::from_str(source).map_err(|error| {
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
    })?;
    if !value.is_object() {
        return Err(ParseIssue::new(
            "JSON value must have an object root",
            SourceSpan {
                start: offset,
                end: offset,
            },
        ));
    }
    let mut collector = Collector {
        source,
        offset: 0,
        property_order: BTreeMap::new(),
    };
    collector.collect_value("").map_err(|local| {
        ParseIssue::new(
            "Invalid JSON token",
            SourceSpan {
                start: offset + local,
                end: offset + local,
            },
        )
    })?;
    Ok((value, collector.property_order))
}

struct Collector<'a> {
    source: &'a str,
    offset: usize,
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
            Some(b'{') => self.collect_object(pointer),
            Some(b'[') => self.collect_array(pointer),
            Some(b'"') => self.read_string().map(|_| ()),
            Some(_) => {
                // Scalars have already been validated by serde_json.
                while self.source.as_bytes().get(self.offset).is_some_and(|byte| {
                    !byte.is_ascii_whitespace() && !matches!(byte, b',' | b']' | b'}')
                }) {
                    self.offset += 1;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nested_arrays_and_escaped_json_pointers_preserve_source_order() {
        let (_, order) = parse(
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
        let (value, order) = parse(
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
        let (_, order) = parse(r#"{"a":[{"old":{}}],"a/b":{"keep":1},"a":null}"#, 0)
            .expect("valid duplicate scalar replacement");
        assert_eq!(order.len(), 2);
        assert_eq!(order["/a~1b"], vec!["keep"]);
    }

    #[test]
    fn strings_do_not_expose_delimiters_and_invalid_json_reports_source_offset() {
        let (_, order) = parse(
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
}
