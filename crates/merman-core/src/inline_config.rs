use serde_json::Value;
#[cfg(any(not(feature = "full-config"), test))]
use serde_json::{Map, Number};

pub(crate) fn parse_mermaid_inline_object(input: &str) -> Result<Value, String> {
    #[cfg(feature = "full-config")]
    {
        parse_yaml_object_body(input)
    }

    #[cfg(not(feature = "full-config"))]
    {
        parse_object_body(input)
    }
}

#[cfg(any(not(feature = "full-config"), test))]
pub(crate) fn parse_inline_config_value(input: &str) -> Result<Value, String> {
    let mut parser = Parser::new(input);
    let value = parser.parse_value()?;
    parser.skip_ws();
    if parser.is_eof() {
        Ok(value)
    } else {
        Err(parser.error("unexpected trailing input"))
    }
}

#[cfg(any(not(feature = "full-config"), test))]
pub(crate) fn parse_object_body(input: &str) -> Result<Value, String> {
    let mut parser = Parser::new(input);
    let value = Value::Object(parser.parse_object_entries(None)?);
    parser.skip_ws();
    if parser.is_eof() {
        Ok(value)
    } else {
        Err(parser.error("unexpected trailing input"))
    }
}

pub(crate) fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub(crate) fn value_to_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(feature = "full-config")]
fn parse_yaml_object_body(input: &str) -> Result<Value, String> {
    let yaml_data = if input.contains('\n') {
        format!("{input}\n")
    } else {
        format!("{{\n{input}\n}}")
    };
    crate::yaml_config::parse_yaml_value(&yaml_data, crate::MAX_DIAGRAM_NESTING_DEPTH)
}

#[cfg(any(not(feature = "full-config"), test))]
struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

#[cfg(any(not(feature = "full-config"), test))]
impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        match self.peek_char() {
            Some('{') => {
                self.next_char();
                Ok(Value::Object(self.parse_object_entries(Some('}'))?))
            }
            Some('[') => {
                self.next_char();
                self.parse_array()
            }
            Some('|') => self.parse_literal_block_scalar(),
            Some('"') | Some('\'') => self.parse_quoted_string().map(Value::String),
            Some(_) => self.parse_bare_value(),
            None => Err(self.error("expected value")),
        }
    }

    fn parse_object_entries(
        &mut self,
        terminator: Option<char>,
    ) -> Result<Map<String, Value>, String> {
        let mut out = Map::new();

        loop {
            self.skip_ws_and_commas();
            if let Some(end) = terminator
                && self.peek_char() == Some(end)
            {
                self.next_char();
                return Ok(out);
            }
            if self.is_eof() {
                return if terminator.is_some() {
                    Err(self.error("unterminated object"))
                } else {
                    Ok(out)
                };
            }

            let key = self.parse_key()?;
            self.skip_ws();
            if self.next_char() != Some(':') {
                return Err(self.error("expected ':' after key"));
            }
            let value = self.parse_value()?;
            if let Some(old) = out.insert(key, value) {
                crate::config::drop_value_nonrecursive(old);
            }
        }
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        let mut out = Vec::new();
        loop {
            self.skip_ws_and_commas();
            match self.peek_char() {
                Some(']') => {
                    self.next_char();
                    return Ok(Value::Array(out));
                }
                Some(_) => out.push(self.parse_value()?),
                None => return Err(self.error("unterminated array")),
            }
        }
    }

    fn parse_key(&mut self) -> Result<String, String> {
        self.skip_ws();
        if matches!(self.peek_char(), Some('"') | Some('\'')) {
            return self.parse_quoted_string();
        }

        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == ':' {
                break;
            }
            if ch == '\n' || ch == '\r' || ch == '}' || ch == ']' {
                return Err(self.error("expected ':' after key"));
            }
            self.next_char();
        }
        let key = self.input[start..self.pos].trim();
        if key.is_empty() {
            Err(self.error("expected key"))
        } else {
            Ok(key.to_string())
        }
    }

    fn parse_quoted_string(&mut self) -> Result<String, String> {
        let Some(quote @ ('"' | '\'')) = self.next_char() else {
            return Err(self.error("expected quoted string"));
        };
        let mut out = String::new();

        while let Some(ch) = self.next_char() {
            if ch == quote {
                return Ok(out);
            }
            if ch != '\\' {
                out.push(ch);
                continue;
            }

            let Some(escaped) = self.next_char() else {
                return Err(self.error("unterminated escape"));
            };
            match escaped {
                '"' => out.push('"'),
                '\'' => out.push('\''),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'b' => out.push('\u{0008}'),
                'f' => out.push('\u{000c}'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'u' => out.push(self.parse_unicode_escape()?),
                other => out.push(other),
            }
        }

        Err(self.error("unterminated string"))
    }

    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let start = self.pos;
        for _ in 0..4 {
            let Some(ch) = self.next_char() else {
                return Err(self.error("unterminated unicode escape"));
            };
            if !ch.is_ascii_hexdigit() {
                return Err(self.error("invalid unicode escape"));
            }
        }
        let code = u32::from_str_radix(&self.input[start..self.pos], 16)
            .map_err(|_| self.error("invalid unicode escape"))?;
        char::from_u32(code).ok_or_else(|| self.error("invalid unicode scalar"))
    }

    fn parse_bare_value(&mut self) -> Result<Value, String> {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if matches!(ch, ',' | '\n' | '\r' | '}' | ']') {
                break;
            }
            self.next_char();
        }

        let raw = self.input[start..self.pos].trim();
        if raw.is_empty() {
            return Err(self.error("expected value"));
        }
        Ok(parse_bare_scalar(raw))
    }

    fn parse_literal_block_scalar(&mut self) -> Result<Value, String> {
        self.next_char();
        while let Some(ch) = self.peek_char() {
            self.next_char();
            if ch == '\n' {
                break;
            }
            if ch == '\r' {
                if self.peek_char() == Some('\n') {
                    self.next_char();
                }
                break;
            }
        }

        let bytes = self.input.as_bytes();
        let mut probe = self.pos;
        let mut block_indent = None;
        let mut out = String::new();

        while probe < self.input.len() {
            let line_start = probe;
            let mut line_end = probe;
            while line_end < self.input.len() && !matches!(bytes[line_end], b'\n' | b'\r') {
                line_end += 1;
            }

            let mut after_line = line_end;
            let had_newline = if after_line < self.input.len() {
                if bytes[after_line] == b'\r'
                    && after_line + 1 < self.input.len()
                    && bytes[after_line + 1] == b'\n'
                {
                    after_line += 2;
                } else {
                    after_line += 1;
                }
                true
            } else {
                false
            };

            let line = &self.input[line_start..line_end];
            let is_blank = line.trim().is_empty();
            let indent = leading_space_count(line);
            if block_indent.is_none() {
                if is_blank {
                    if had_newline {
                        out.push('\n');
                        probe = after_line;
                        continue;
                    }
                    break;
                }
                block_indent = Some(indent);
            }

            let indent = block_indent.unwrap_or(0);
            if !is_blank && leading_space_count(line) < indent {
                break;
            }
            if !is_blank {
                out.push_str(&line[indent.min(line.len())..]);
            }
            if had_newline {
                out.push('\n');
            }
            probe = after_line;
        }

        self.pos = probe;
        Ok(Value::String(out))
    }

    fn skip_ws(&mut self) {
        while self.peek_char().is_some_and(char::is_whitespace) {
            self.next_char();
        }
    }

    fn skip_ws_and_commas(&mut self) {
        while self
            .peek_char()
            .is_some_and(|ch| ch.is_whitespace() || ch == ',')
        {
            self.next_char();
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn error(&self, message: &str) -> String {
        format!("{message} at byte {}", self.pos)
    }
}

#[cfg(any(not(feature = "full-config"), test))]
fn parse_bare_scalar(raw: &str) -> Value {
    match raw {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        "null" => return Value::Null,
        _ => {}
    }

    if raw
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_digit() || *b == b'-')
        && let Ok(Value::Number(n)) = serde_json::from_str::<Value>(raw)
        && number_is_finite(&n)
    {
        return Value::Number(n);
    }

    Value::String(raw.to_string())
}

#[cfg(any(not(feature = "full-config"), test))]
fn number_is_finite(n: &Number) -> bool {
    n.as_f64().is_some_and(f64::is_finite)
}

#[cfg(any(not(feature = "full-config"), test))]
fn leading_space_count(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ')
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_common_mermaid_inline_object_body() {
        let value = parse_object_body(r#" shape: rounded, label: "End", flag: true, size: 42 "#)
            .expect("inline object parses");

        assert_eq!(
            value,
            json!({
                "shape": "rounded",
                "label": "End",
                "flag": true,
                "size": 42
            })
        );
    }

    #[test]
    fn parses_multiline_body_without_commas() {
        let value = parse_object_body(
            r#"
shape: circle
other: "clock"
asset: MC-1234
"#,
        )
        .expect("multiline object parses");

        assert_eq!(
            value,
            json!({
                "shape": "circle",
                "other": "clock",
                "asset": "MC-1234"
            })
        );
    }

    #[test]
    fn parses_literal_block_scalar() {
        let value = parse_object_body(
            r#"
label: |
  This is a
  multiline string
other: clock
"#,
        )
        .expect("literal block scalar parses");

        assert_eq!(
            value,
            json!({
                "label": "This is a\nmultiline string\n",
                "other": "clock"
            })
        );
    }

    #[test]
    fn parses_directive_style_value() {
        let value = parse_inline_config_value(
            r#"{ theme: 'base', flowchart: { htmlLabels: true }, list: [1, "two", false] }"#,
        )
        .expect("directive object parses");

        assert_eq!(
            value,
            json!({
                "theme": "base",
                "flowchart": {
                    "htmlLabels": true
                },
                "list": [1, "two", false]
            })
        );
    }

    #[test]
    fn rejects_missing_colon() {
        let err = parse_object_body(r#""type" "control""#).expect_err("invalid object fails");
        assert!(err.contains("expected ':'"), "unexpected error: {err}");
    }
}
