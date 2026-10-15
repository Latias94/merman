use crate::mermaid_style::{
    parse_style_declaration, strip_css_important,
    visit_style_declaration_boundaries_with_checkpoints,
};

/// Reads the final declaration for one non-metric HTML fallback helper property.
/// Typography itself is resolved by `cascade`, which retains source context and
/// cascade priority.
pub(super) fn extract_style_property_with_checkpoints<E>(
    style: &str,
    property: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    let mut found = None;
    visit_style_declaration_boundaries_with_checkpoints(style, checkpoint, |boundary| {
        if let Some(declaration) = parse_style_declaration(boundary.raw())
            && declaration.property().eq_ignore_ascii_case(property)
            && !declaration.value().is_empty()
        {
            found = Some(declaration.value().to_string());
        }
        Ok(true)
    })?;
    Ok(found)
}

pub(super) fn parse_css_px_value(value: &str) -> Option<f64> {
    let value = strip_css_important(value);
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

    #[test]
    fn helper_keeps_semicolons_inside_css_strings() {
        let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
        let value = extract_style_property_with_checkpoints(
            "font-family: \"a;b\", sans-serif; width: 20px;",
            "font-family",
            &mut checkpoint,
        )
        .unwrap();
        assert_eq!(value.as_deref(), Some("\"a;b\", sans-serif"));
    }

    #[test]
    fn helper_keeps_escaped_quotes_and_semicolons_inside_css_strings() {
        let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
        let value = extract_style_property_with_checkpoints(
            r#"font-family: "a\";b", sans-serif; width: 20px;"#,
            "font-family",
            &mut checkpoint,
        )
        .unwrap();
        assert_eq!(value.as_deref(), Some(r#""a\";b", sans-serif"#));
    }
}
