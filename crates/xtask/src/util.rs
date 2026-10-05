use crate::XtaskError;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    encode_lower_hex(&Sha256::digest(bytes))
}

pub(crate) fn encode_lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

pub(crate) fn is_canonical_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn read_text(path: &Path) -> Result<String, XtaskError> {
    fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn read_text_normalized(path: &Path) -> Result<String, XtaskError> {
    let text = read_text(path)?;
    let normalized_line_endings = text.replace("\r\n", "\n");
    Ok(normalized_line_endings.trim_end().to_string())
}

pub(crate) fn extract_add_to_set_string_array(
    src: &str,
    ident: &str,
) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = addToSet({{}}, [");
    extract_dompurify_array_definition(src, ident, &needle)
}

pub(crate) fn extract_frozen_string_array(
    src: &str,
    ident: &str,
) -> Result<Vec<String>, XtaskError> {
    let needle = format!("const {ident} = freeze([");
    extract_dompurify_array_definition(src, ident, &needle)
}

fn extract_dompurify_array_definition(
    src: &str,
    ident: &str,
    needle: &str,
) -> Result<Vec<String>, XtaskError> {
    let mut definitions = src.match_indices(needle);
    let (start, _) = definitions
        .next()
        .ok_or_else(|| XtaskError::ParseDompurify(format!("missing {ident} definition")))?;
    if definitions.next().is_some() {
        return Err(XtaskError::ParseDompurify(format!(
            "duplicate {ident} definition"
        )));
    }
    let (values, remaining) = parse_literal_string_array(src, start + needle.len() - 1)?;
    if !remaining.trim_start().starts_with(");") {
        return Err(XtaskError::ParseDompurify(format!(
            "unsupported {ident} initializer after its literal array"
        )));
    }
    Ok(values)
}

pub(crate) fn extract_string_array_at(
    src: &str,
    bracket_start: usize,
) -> Result<Vec<String>, XtaskError> {
    parse_literal_string_array(src, bracket_start).map(|(values, _)| values)
}

// Read the nonempty literal lists used by DOMPurify and Mermaid's ELK registry.
// Deliberately reject other JavaScript syntax instead of evaluating or skipping it.
fn parse_literal_string_array(
    src: &str,
    bracket_start: usize,
) -> Result<(Vec<String>, &str), XtaskError> {
    let mut remaining = src
        .get(bracket_start..)
        .and_then(|suffix| suffix.strip_prefix('['))
        .ok_or_else(|| XtaskError::ParseDompurify("expected array '['".to_string()))?;
    let mut values = Vec::new();
    loop {
        remaining = remaining.trim_start_matches(|c: char| c.is_ascii_whitespace());
        if let Some(suffix) = remaining.strip_prefix(']') {
            if values.is_empty() {
                return Err(XtaskError::ParseDompurify(
                    "expected a nonempty literal string array".to_string(),
                ));
            }
            return Ok((values, suffix));
        }
        let mut chars = remaining.chars();
        let quote = match chars.next() {
            Some(quote @ ('\'' | '"')) => quote,
            None => return Err(XtaskError::ParseDompurify("unterminated array".to_string())),
            _ => {
                return Err(XtaskError::ParseDompurify(
                    "expected a quoted string literal in array".to_string(),
                ));
            }
        };
        let mut value = String::new();
        loop {
            match chars.next() {
                Some(c) if c == quote => break,
                Some('\\') => match chars.next() {
                    Some(escaped @ ('\'' | '"' | '\\')) => value.push(escaped),
                    _ => {
                        return Err(XtaskError::ParseDompurify(
                            "unsupported or unterminated string escape".to_string(),
                        ));
                    }
                },
                Some('\n' | '\r' | '\u{2028}' | '\u{2029}') => {
                    return Err(XtaskError::ParseDompurify(
                        "unescaped line break in string literal".to_string(),
                    ));
                }
                Some(c) => value.push(c),
                None => {
                    return Err(XtaskError::ParseDompurify(
                        "unterminated string literal".to_string(),
                    ));
                }
            }
        }
        if value.is_empty() {
            return Err(XtaskError::ParseDompurify(
                "expected a nonempty string literal".to_string(),
            ));
        }
        values.push(value);
        remaining = chars
            .as_str()
            .trim_start_matches(|c: char| c.is_ascii_whitespace());
        if let Some(suffix) = remaining.strip_prefix(']') {
            return Ok((values, suffix));
        }
        remaining = remaining.strip_prefix(',').ok_or_else(|| {
            XtaskError::ParseDompurify("expected ',' or ']' after string literal".to_string())
        })?;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_add_to_set_string_array, extract_frozen_string_array, extract_string_array_at,
        sha256_hex,
    };

    #[test]
    fn sha256_hex_preserves_canonical_bytes_and_leading_zeroes() {
        for (input, expected) in [
            (
                b"".as_slice(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"286".as_slice(),
                "00328ce57bbc14b33bd6695bc8eb32cdf2fb5f3a7d89ec14a42825e15d39df60",
            ),
            (
                b"\0\xff\x80\0".as_slice(),
                "f11b18659845b5c27e6040a28586bbe79db7c24d4eda7930381151bd4602490d",
            ),
        ] {
            assert_eq!(sha256_hex(input), expected);
        }
    }

    #[test]
    fn dompurify_literal_arrays_accept_old_and_rolldown_formats() {
        for array in [
            "['a', 'svg', 'feBlend']",
            "[\n\t\"a\",\n\t\"svg\",\n\t\"feBlend\",\n]",
        ] {
            assert_eq!(
                extract_frozen_string_array(&format!("const html$1 = freeze({array});"), "html$1")
                    .expect("frozen literal list"),
                ["a", "svg", "feBlend"],
            );
            assert_eq!(
                extract_add_to_set_string_array(
                    &format!("const DEFAULT_DATA_URI_TAGS = addToSet({{}}, {array});"),
                    "DEFAULT_DATA_URI_TAGS",
                )
                .expect("addToSet literal list"),
                ["a", "svg", "feBlend"],
            );
        }
    }

    #[test]
    fn literal_arrays_preserve_unicode_quotes_escapes_and_string_delimiters() {
        assert_eq!(
            extract_string_array_at(r#"['café😀', "a'b", 'a"b', '\\', '\'', "\"", '],)']"#, 0)
                .expect("supported literal strings"),
            ["café😀", "a'b", "a\"b", "\\", "'", "\"", "],)"],
        );
    }

    #[test]
    fn literal_arrays_reject_partial_or_unsupported_inputs() {
        for source in [
            "[]",
            "[ \n ]",
            "['']",
            "[\"\"]",
            "[value]",
            "['a', value]",
            "[value, 'a']",
            "['a' 'b']",
            "['a',, 'b']",
            "['a' + 'b']",
            "[['a']]",
            "[...['a']]",
            "[`a`]",
            "['a', /* later */ 'b']",
            "['a",
            "[\"a'",
            "['a'",
            "['a',",
            "['a\\",
            "['a\nb']",
            "['a\rb']",
            "['a\u{2028}b']",
            "['a\u{2029}b']",
            r"['\n']",
            r"['\x61']",
            r"['\u0061']",
            r"['\q']",
        ] {
            assert!(
                extract_string_array_at(source, 0).is_err(),
                "accepted {source:?}"
            );
        }
        assert!(extract_string_array_at("é['a']", 1).is_err());
        assert!(extract_string_array_at("['a']", usize::MAX).is_err());
    }

    #[test]
    fn dompurify_literal_arrays_require_one_complete_initializer() {
        for source in [
            "const other = freeze(['a']);",
            "const html = freeze(['a']); const html = freeze(['b']);",
            "const html = freeze(['a'].concat(['b']));",
            "const html = freeze(['a'], extra);",
            "const html = freeze(['a']",
        ] {
            assert!(
                extract_frozen_string_array(source, "html").is_err(),
                "accepted {source:?}"
            );
        }
        assert!(
            extract_add_to_set_string_array(
                "const SAFE = addToSet({}, ['a'].concat(['b']));",
                "SAFE",
            )
            .is_err()
        );
    }

    #[test]
    fn literal_arrays_preserve_mermaid_elk_algorithm_format() {
        let source =
            "export const ELK_ALGORITHMS = [\n  'elk.stress',\n  'elk.force',\n] as const;";
        assert_eq!(
            extract_string_array_at(source, source.find('[').expect("array start"))
                .expect("ELK algorithm list"),
            ["elk.stress", "elk.force"],
        );
    }
}
