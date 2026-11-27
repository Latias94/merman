use crate::MermaidConfig;
use crate::diagrams::scan::is_ecmascript_whitespace;
use crate::sanitize::sanitize_text;
#[cfg(feature = "diagram-packet")]
pub(crate) use merman_core_preprocess::scan::langium::LangiumCommonDbFields;
#[cfg(feature = "presentation")]
use serde_json::{Map, Value};

fn strip_leading_whitespace(s: &str) -> String {
    s.trim_start_matches(is_ecmascript_whitespace).to_string()
}

fn collapse_newline_whitespace(s: &str) -> String {
    // Mermaid's commonDb.ts: `sanitizeText(txt).replace(/\n\s+/g, '\n')`
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        out.push(ch);
        if ch == '\n' {
            while chars.peek().is_some_and(|c| is_ecmascript_whitespace(*c)) {
                chars.next();
            }
        }
    }
    out
}

#[cfg(feature = "presentation")]
fn sanitize_string_field<F>(obj: &mut Map<String, Value>, key: &'static str, transform: F)
where
    F: FnOnce(&str) -> String,
{
    let Some(v) = obj.get_mut(key) else {
        return;
    };
    let Value::String(s) = v else {
        return;
    };
    *s = transform(s);
}

#[cfg(feature = "presentation")]
pub(crate) fn apply_common_db_sanitization(model: &mut Value, config: &MermaidConfig) {
    let Value::Object(obj) = model else {
        return;
    };

    sanitize_string_field(obj, "title", |s| sanitize_text(s, config));
    sanitize_string_field(obj, "accTitle", |s| sanitize_acc_title(s, config));
    sanitize_string_field(obj, "accDescr", |s| sanitize_acc_descr(s, config));
}

pub(crate) fn sanitize_acc_title(s: &str, config: &MermaidConfig) -> String {
    strip_leading_whitespace(&sanitize_text(s, config))
}

pub(crate) fn sanitize_acc_descr(s: &str, config: &MermaidConfig) -> String {
    collapse_newline_whitespace(&sanitize_text(s, config))
}

#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) fn sanitize_optional_title(value: &mut Option<String>, config: &MermaidConfig) {
    if let Some(s) = value.as_deref() {
        *value = Some(sanitize_text(s, config));
    }
}

#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) fn sanitize_optional_acc_title(value: &mut Option<String>, config: &MermaidConfig) {
    if let Some(s) = value.as_deref() {
        *value = Some(sanitize_acc_title(s, config));
    }
}

#[allow(
    dead_code,
    reason = "Shared parser facilities have different consumers in each family selection."
)]
pub(crate) fn sanitize_optional_acc_descr(value: &mut Option<String>, config: &MermaidConfig) {
    if let Some(s) = value.as_deref() {
        *value = Some(sanitize_acc_descr(s, config));
    }
}

#[cfg(test)]
mod tests {
    use super::{collapse_newline_whitespace, strip_leading_whitespace};

    #[test]
    fn accessibility_whitespace_matches_common_db_javascript_regexes() {
        assert_eq!(strip_leading_whitespace("\u{feff}\u{00a0}Title "), "Title ");
        assert_eq!(strip_leading_whitespace("\u{0085}Title"), "\u{0085}Title");
        assert_eq!(
            collapse_newline_whitespace("First\n\u{feff}\u{00a0}Second"),
            "First\nSecond"
        );
        assert_eq!(
            collapse_newline_whitespace("First\n\u{0085}Second"),
            "First\n\u{0085}Second"
        );
    }
}
