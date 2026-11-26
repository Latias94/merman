use merman_model::SourceSpan;

#[cfg(feature = "editor-facts")]
pub fn shape_value_expected_span(code: &str, start: usize, end: usize) -> Option<SourceSpan> {
    let raw = code.get(start..end)?;
    let body = raw.strip_prefix("@{")?;
    let body = body.strip_suffix('}').unwrap_or(body);
    let body_base = start + 2;
    let mut pos = 0usize;
    let mut in_string: Option<char> = None;
    let mut depth = 0usize;

    while pos < body.len() {
        let Some(ch) = body[pos..].chars().next() else {
            break;
        };
        if let Some(quote) = in_string {
            if ch == '\\' {
                pos += ch.len_utf8();
                if pos < body.len() {
                    let Some(escaped) = body[pos..].chars().next() else {
                        break;
                    };
                    pos += escaped.len_utf8();
                }
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            pos += ch.len_utf8();
            continue;
        }

        match ch {
            '"' | '\'' => {
                in_string = Some(ch);
                pos += ch.len_utf8();
            }
            '{' | '[' | '(' => {
                depth += 1;
                pos += ch.len_utf8();
            }
            '}' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                pos += ch.len_utf8();
            }
            ']' | ')' => {
                depth = depth.saturating_sub(1);
                pos += ch.len_utf8();
            }
            _ => {
                if depth == 0 && body[pos..].starts_with("shape") && shape_key_boundary(body, pos) {
                    let mut key_end = pos + "shape".len();
                    while let Some(ch) = body[key_end..].chars().next() {
                        if ch.is_whitespace() {
                            key_end += ch.len_utf8();
                        } else {
                            break;
                        }
                    }
                    if body[key_end..].starts_with(':') {
                        let mut value_start = key_end + 1;
                        while let Some(ch) = body[value_start..].chars().next() {
                            if ch.is_whitespace() {
                                value_start += ch.len_utf8();
                            } else {
                                break;
                            }
                        }
                        let value_end = shape_value_end(body, value_start);
                        return Some(SourceSpan::new(
                            body_base + value_start,
                            body_base + value_end,
                        ));
                    }
                }
                pos += ch.len_utf8();
            }
        }
    }

    None
}

#[cfg(feature = "editor-facts")]
fn shape_key_boundary(body: &str, pos: usize) -> bool {
    let before = if pos == 0 {
        None
    } else {
        body[..pos].chars().next_back()
    };
    let after = body[pos + "shape".len()..].chars().next();

    before.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
        && after.is_none_or(|ch| ch.is_whitespace() || ch == ':')
}

#[cfg(feature = "editor-facts")]
fn shape_value_end(body: &str, start: usize) -> usize {
    if start >= body.len() {
        return start;
    }

    let Some(first) = body[start..].chars().next() else {
        return start;
    };

    match first {
        '"' | '\'' => {
            let quote = first;
            let mut pos = start + 1;
            while pos < body.len() {
                let Some(ch) = body[pos..].chars().next() else {
                    break;
                };
                if ch == '\\' {
                    pos += ch.len_utf8();
                    if pos < body.len() {
                        let Some(escaped) = body[pos..].chars().next() else {
                            break;
                        };
                        pos += escaped.len_utf8();
                    }
                    continue;
                }
                if ch == quote {
                    pos += ch.len_utf8();
                    break;
                }
                pos += ch.len_utf8();
            }
            pos
        }
        _ => {
            let mut pos = start;
            while pos < body.len() {
                let Some(ch) = body[pos..].chars().next() else {
                    break;
                };
                match ch {
                    ',' | '}' | '\n' | '\r' | ' ' | '\t' => break,
                    _ => pos += ch.len_utf8(),
                }
            }
            pos
        }
    }
}
