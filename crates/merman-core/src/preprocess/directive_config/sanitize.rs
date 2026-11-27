//! Admission checks for values authored through Mermaid directives.

use crate::{OperationControl, OperationControlResult};
use merman_model::text::is_ecmascript_whitespace;
use serde_json::Value;

#[derive(Clone)]
enum DirectiveValuePathSegment {
    Key(String),
    Index(usize),
}

#[derive(Clone, Copy)]
enum DirectiveDictionaryKind {
    NodeColors,
    IconReferences,
}

#[cfg(test)]
pub(in crate::preprocess) fn sanitize_directive(value: &mut Value) {
    let control = OperationControl::new();
    sanitize_directive_controlled(value, &control)
        .expect("a private parse control cannot be cancelled");
}

pub(in crate::preprocess) fn sanitize_directive_controlled(
    value: &mut Value,
    control: &OperationControl,
) -> OperationControlResult<()> {
    control.checkpoint()?;
    let mut stack = vec![Vec::<DirectiveValuePathSegment>::new()];
    let mut visited = 0usize;

    while let Some(path) = stack.pop() {
        if visited.is_multiple_of(64) {
            control.checkpoint()?;
        }
        visited = visited.saturating_add(1);
        let Some(current) = directive_value_at_path_mut(value, &path) else {
            continue;
        };

        match current {
            Value::Object(map) => {
                if let Some(old) = map.remove("secure") {
                    crate::json_value::drop_value_nonrecursive(old);
                }

                let blocked_keys = map
                    .iter()
                    .filter(|(key, value)| {
                        is_suspicious_directive_key(key)
                            || !crate::generated::is_default_config_key(key)
                            || value.is_null()
                    })
                    .map(|(key, _)| key)
                    .cloned()
                    .collect::<Vec<_>>();
                for key in blocked_keys {
                    if let Some(old) = map.remove(&key) {
                        crate::json_value::drop_value_nonrecursive(old);
                    }
                }

                let child_keys = map.keys().cloned().collect::<Vec<_>>();
                for key in child_keys.into_iter().rev() {
                    if let Some(kind) = directive_dictionary_kind(&key)
                        && map
                            .get_mut(&key)
                            .is_some_and(|child| sanitize_directive_dictionary(child, kind))
                    {
                        continue;
                    }

                    let mut child_path = path.clone();
                    child_path.push(DirectiveValuePathSegment::Key(key));
                    stack.push(child_path);
                }
            }
            Value::Array(arr) => {
                for idx in (0..arr.len()).rev() {
                    let mut child_path = path.clone();
                    child_path.push(DirectiveValuePathSegment::Index(idx));
                    stack.push(child_path);
                }
            }
            Value::String(s) => {
                // CSS values share the final source admission boundary. Keep their original
                // spelling here so that boundary can reject the override and preserve host values.
                if path.iter().any(|segment| {
                    matches!(segment, DirectiveValuePathSegment::Key(key)
                        if crate::config::is_presentation_field(key))
                }) {
                    continue;
                }
                if directive_path_is_css(&path) && !css_braces_are_balanced(s) {
                    *s = "{ /* ERROR: Unbalanced CSS */ }".to_string();
                }
                // Other strings also reach that shared boundary unchanged. Clearing a rejected
                // value here would create an empty override instead of the upstream fallback.
            }
            _ => {}
        }
    }
    control.checkpoint()
}

fn directive_path_is_css(path: &[DirectiveValuePathSegment]) -> bool {
    matches!(
        path.last(),
        Some(DirectiveValuePathSegment::Key(key))
            if ["themeCSS", "fontFamily", "altFontFamily"]
                .iter()
                .any(|css_key| key.contains(css_key))
    )
}

fn is_suspicious_directive_key(key: &str) -> bool {
    key.starts_with("__") || key.contains("proto") || key.contains("constr")
}

fn css_braces_are_balanced(css: &str) -> bool {
    let mut depth = 0usize;
    for ch in css.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                let Some(next) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next;
            }
            _ => {}
        }
    }
    depth == 0
}

fn directive_dictionary_kind(key: &str) -> Option<DirectiveDictionaryKind> {
    // Source: Mermaid 11.16 `sanitizeDirective.ts` DICTIONARY_CONFIG_PATTERNS.
    match key {
        "nodeColors" => Some(DirectiveDictionaryKind::NodeColors),
        "filenameIcons" | "extensionIcons" => Some(DirectiveDictionaryKind::IconReferences),
        _ => None,
    }
}

fn sanitize_directive_dictionary(value: &mut Value, kind: DirectiveDictionaryKind) -> bool {
    let is_valid_value = |value: &Value| {
        value.as_str().is_some_and(|value| match kind {
            DirectiveDictionaryKind::NodeColors => is_valid_node_color(value),
            DirectiveDictionaryKind::IconReferences => is_valid_icon_reference(value),
        })
    };

    match value {
        Value::Object(map) => {
            let blocked_keys = map
                .iter()
                .filter(|(key, value)| is_suspicious_dictionary_key(key) || !is_valid_value(value))
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();

            for key in blocked_keys {
                if let Some(old) = map.remove(&key) {
                    crate::json_value::drop_value_nonrecursive(old);
                }
            }
            true
        }
        Value::Array(values) => {
            for value in values.iter_mut().filter(|value| !is_valid_value(value)) {
                let old = std::mem::replace(value, Value::Null);
                crate::json_value::drop_value_nonrecursive(old);
            }
            true
        }
        _ => false,
    }
}

fn is_suspicious_dictionary_key(key: &str) -> bool {
    is_suspicious_directive_key(key)
}

fn is_valid_icon_reference(value: &str) -> bool {
    let mut segments = value.split(':');
    let Some(first) = segments.next() else {
        return false;
    };
    let second = segments.next();

    is_valid_icon_reference_segment(first)
        && second.is_none_or(is_valid_icon_reference_segment)
        && segments.next().is_none()
}

fn is_valid_icon_reference_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn is_valid_node_color(value: &str) -> bool {
    if let Some(hex) = value.strip_prefix('#') {
        return (3..=8).contains(&hex.len()) && hex.bytes().all(|byte| byte.is_ascii_hexdigit());
    }

    if is_valid_node_color_function(value, "rgb(") || is_valid_node_color_function(value, "hsl(") {
        return true;
    }

    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_valid_node_color_function(value: &str, prefix: &str) -> bool {
    let Some(actual_prefix) = value.get(..prefix.len()) else {
        return false;
    };
    if !actual_prefix.eq_ignore_ascii_case(prefix) || !value.ends_with(')') {
        return false;
    }

    let inner = &value[prefix.len()..value.len() - 1];
    !inner.is_empty()
        && inner.chars().all(|ch| {
            ch.is_ascii_digit() || is_ecmascript_whitespace(ch) || matches!(ch, '%' | ',' | '.')
        })
}

fn directive_value_at_path_mut<'a>(
    mut value: &'a mut Value,
    path: &[DirectiveValuePathSegment],
) -> Option<&'a mut Value> {
    for segment in path {
        match segment {
            DirectiveValuePathSegment::Key(key) => {
                value = value.as_object_mut()?.get_mut(key)?;
            }
            DirectiveValuePathSegment::Index(idx) => {
                value = value.as_array_mut()?.get_mut(*idx)?;
            }
        }
    }
    Some(value)
}
