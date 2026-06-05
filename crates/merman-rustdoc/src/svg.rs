use crate::error::{Error, Result};
use crate::options::SanitizeMode;

pub(crate) fn validate_svg(svg: &str, mode: SanitizeMode) -> Result<()> {
    if mode == SanitizeMode::Off {
        return Ok(());
    }

    let document = roxmltree::Document::parse(svg)
        .map_err(|err| Error::new(format!("rendered SVG is not valid XML: {err}")))?;

    for node in document.descendants().filter(roxmltree::Node::is_element) {
        let element = node.tag_name().name();
        if element.eq_ignore_ascii_case("script") {
            return Err(Error::new(
                "strict SVG sanitization rejected a <script> element",
            ));
        }

        for attr in node.attributes() {
            let name = attr.name();
            let value = attr.value();
            if is_event_attr(name) {
                return Err(Error::new(format!(
                    "strict SVG sanitization rejected event attribute `{name}`"
                )));
            }
            if is_href_attr(name) {
                validate_href(element, name, value)?;
            }
            validate_css_urls(name, value)?;
        }
    }

    Ok(())
}

fn is_event_attr(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('o') | Some('O')) && matches!(chars.next(), Some('n') | Some('N'))
}

fn is_href_attr(name: &str) -> bool {
    name.eq_ignore_ascii_case("href") || name.eq_ignore_ascii_case("xlink:href")
}

fn validate_href(element: &str, name: &str, value: &str) -> Result<()> {
    let normalized = normalize_url(value);
    if normalized.starts_with("javascript:") {
        return Err(Error::new(format!(
            "strict SVG sanitization rejected `{name}` with a javascript: URL"
        )));
    }

    if is_resource_element(element) && !normalized.starts_with('#') {
        return Err(Error::new(format!(
            "strict SVG sanitization rejected remote or embedded resource reference on <{element}>"
        )));
    }

    Ok(())
}

fn is_resource_element(element: &str) -> bool {
    element.eq_ignore_ascii_case("image") || element.eq_ignore_ascii_case("use")
}

fn validate_css_urls(name: &str, value: &str) -> Result<()> {
    let lower = value.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(start) = rest.find("url(") {
        rest = &rest[start + 4..];
        let Some(end) = rest.find(')') else {
            return Err(Error::new(format!(
                "strict SVG sanitization rejected malformed CSS url() in `{name}`"
            )));
        };
        let target = rest[..end]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim();
        if !target.starts_with('#') {
            return Err(Error::new(format!(
                "strict SVG sanitization rejected external CSS url() reference in `{name}`"
            )));
        }
        rest = &rest[end + 1..];
    }

    Ok(())
}

fn normalize_url(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_mode_allows_local_marker_urls() {
        validate_svg(
            r##"<svg><path marker-end="url(#arrow)"/><defs><marker id="arrow"/></defs></svg>"##,
            SanitizeMode::Strict,
        )
        .unwrap();
    }

    #[test]
    fn strict_mode_rejects_script_elements() {
        let err = validate_svg(
            r#"<svg><script>alert(1)</script></svg>"#,
            SanitizeMode::Strict,
        )
        .unwrap_err();

        assert!(err.to_string().contains("<script>"));
    }

    #[test]
    fn strict_mode_rejects_event_attributes() {
        let err = validate_svg(
            r#"<svg><g onclick="alert(1)"/></svg>"#,
            SanitizeMode::Strict,
        )
        .unwrap_err();

        assert!(err.to_string().contains("onclick"));
    }

    #[test]
    fn strict_mode_rejects_javascript_links() {
        let err = validate_svg(
            r#"<svg><a href="java script:alert(1)"/></svg>"#,
            SanitizeMode::Strict,
        )
        .unwrap_err();

        assert!(err.to_string().contains("javascript"));
    }

    #[test]
    fn strict_mode_rejects_remote_image_resources() {
        let err = validate_svg(
            r#"<svg><image href="https://example.com/a.png"/></svg>"#,
            SanitizeMode::Strict,
        )
        .unwrap_err();

        assert!(err.to_string().contains("<image>"));
    }

    #[test]
    fn off_mode_skips_validation() {
        validate_svg(r#"<svg><script>alert(1)</script></svg>"#, SanitizeMode::Off).unwrap();
    }
}
