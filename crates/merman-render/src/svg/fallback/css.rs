use crate::svg::pipeline::{checkpoint_loop, trim_with_checkpoints};

/// Reads the final declaration for one non-metric HTML fallback helper property.
/// Typography itself is resolved by `cascade`, which retains source context and
/// cascade priority.
pub(super) fn extract_style_property_with_checkpoints<E>(
    style: &str,
    property: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    let mut found = None;
    for (order, declaration) in split_style_declarations(style, checkpoint)?
        .into_iter()
        .enumerate()
    {
        checkpoint_loop(order, checkpoint)?;
        let Some(colon) = declaration.find(':') else {
            continue;
        };
        let name = trim_with_checkpoints(&declaration[..colon], checkpoint)?;
        if !name.eq_ignore_ascii_case(property) {
            continue;
        }
        let value = strip_important(&declaration[colon + 1..]);
        if !value.is_empty() {
            found = Some(value.to_string());
        }
    }
    checkpoint()?;
    Ok(found)
}

pub(super) fn split_style_declarations<'a, E>(
    style: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<&'a str>, E> {
    let mut declarations = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut paren_depth = 0usize;
    for (iteration, (offset, character)) in style.char_indices().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if let Some(current_quote) = quote {
            if character == current_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            ';' if paren_depth == 0 => {
                declarations.push(&style[start..offset]);
                start = offset + character.len_utf8();
            }
            _ => {}
        }
    }
    declarations.push(&style[start..]);
    checkpoint()?;
    Ok(declarations)
}

fn strip_important(value: &str) -> &str {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    let Some(offset) = lower.rfind("!important") else {
        return value;
    };
    if !lower[offset + "!important".len()..].trim().is_empty() {
        return value;
    }
    value[..offset].trim()
}

pub(super) fn parse_css_px_value(value: &str) -> Option<f64> {
    let value = strip_important(value);
    let number = value
        .strip_suffix("px")
        .or_else(|| value.strip_suffix("PX"))
        .unwrap_or(value)
        .trim();
    number
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_reads_later_declarations_without_reintroducing_selector_matching() {
        let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
        let value = extract_style_property_with_checkpoints(
            "width: 10px; width: 20px !important;",
            "width",
            &mut checkpoint,
        )
        .unwrap();
        assert_eq!(value.as_deref(), Some("20px"));
    }
}
