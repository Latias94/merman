use super::limits::IconRegistryResourceLimitId::{MaxNameBytes, MaxPrefixBytes};

/// Parses a reference into bounded, borrowed prefix and name segments.
#[doc(hidden)]
pub fn parse_icon_reference<'a>(
    icon_name: &'a str,
    fallback_prefix: Option<&'a str>,
) -> Option<(&'a str, &'a str)> {
    if icon_name.is_empty() || icon_name.trim() != icon_name {
        return None;
    }

    // Mirror Iconify's `stringToIcon(value, true, allowSimpleName)` ordering. A leading provider
    // is syntax only for this in-process registry: remove it first, then parse colon or dash forms.
    // Read at most four components so malformed input cannot allocate in proportion to the number
    // of separators before it is rejected.
    let mut parts = icon_name.splitn(4, ':');
    let first = parts.next()?;
    let second = parts.next();
    let third = parts.next();
    if parts.next().is_some() {
        return None;
    }

    let provider_present = icon_name.starts_with('@');
    let (qualified, simple_name) = if provider_present {
        match (second, third) {
            (Some(name), None) => (None, Some(name)),
            (Some(prefix), Some(name)) => (Some((prefix, name)), None),
            _ => return None,
        }
    } else {
        match (second, third) {
            (None, None) => (None, Some(first)),
            (Some(name), None) => (Some((first, name)), None),
            (Some(prefix), Some(name)) => (Some((prefix, name)), None),
            (None, Some(_)) => return None,
        }
    };

    if let Some((prefix, name)) = qualified {
        return admitted_reference(prefix, name);
    }

    let icon_name = simple_name?;

    // Iconify parses the dash shorthand before applying a simple-name fallback. In particular,
    // `aws-lambda` names the `aws:lambda` icon even in Architecture, while a truly simple name
    // such as `database` can resolve through `mermaid-architecture`.
    if let Some((prefix, name)) = icon_name.split_once('-') {
        // Invalid dash components also make the whole name invalid. An overlong shorthand
        // prefix remains a miss even when the combined name would fit under the fallback pack.
        return admitted_reference(prefix, name);
    }

    if provider_present {
        return None;
    }
    admitted_reference(fallback_prefix?, icon_name)
}

fn admitted_reference<'a>(prefix: &'a str, name: &'a str) -> Option<(&'a str, &'a str)> {
    (prefix.len() <= MaxPrefixBytes.fixed_value() as usize
        && name.len() <= MaxNameBytes.fixed_value() as usize
        && valid_identifier(prefix)
        && valid_identifier(name))
    .then_some((prefix, name))
}

pub(super) fn valid_identifier(value: &str) -> bool {
    let mut segments = value.split('-');
    let Some(first) = segments.next() else {
        return false;
    };
    valid_identifier_segment(first) && segments.all(valid_identifier_segment)
}

fn valid_identifier_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

pub(super) fn canonical_key(prefix: &str, name: &str) -> String {
    let mut key = String::with_capacity(prefix.len() + name.len() + 1);
    key.push_str(prefix);
    key.push(':');
    key.push_str(name);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dash_shorthand_precedes_fallback_prefix() {
        assert_eq!(
            parse_icon_reference("aws-lambda", Some("mermaid-architecture")),
            Some(("aws", "lambda"))
        );
        assert_eq!(
            parse_icon_reference("database", Some("mermaid-architecture")),
            Some(("mermaid-architecture", "database"))
        );
    }

    #[test]
    fn provider_is_syntax_not_a_registry_namespace() {
        assert_eq!(
            parse_icon_reference("@cloud:logos:aws-lambda", None),
            Some(("logos", "aws-lambda"))
        );
        assert_eq!(
            parse_icon_reference("cloud:logos:aws-lambda", None),
            Some(("logos", "aws-lambda"))
        );
        assert_eq!(
            parse_icon_reference("@cloud:logos-aws", None),
            Some(("logos", "aws"))
        );
        assert_eq!(
            parse_icon_reference("@cloud:logos-aws-lambda", None),
            Some(("logos", "aws-lambda"))
        );
        assert_eq!(
            parse_icon_reference("@cloud:database", Some("mermaid-architecture")),
            None
        );
    }

    #[test]
    fn identifier_grammar_is_lowercase_ascii_segments() {
        for valid in ["a", "a1", "mermaid-architecture", "123"] {
            assert!(valid_identifier(valid), "{valid}");
        }
        for invalid in ["", "A", "a_b", "a.b", "-a", "a-", "a--b", "é"] {
            assert!(!valid_identifier(invalid), "{invalid}");
        }
    }

    #[test]
    fn excessive_colon_components_are_rejected_without_component_collection() {
        assert_eq!(parse_icon_reference("a:b:c:d", None), None);

        let many_components = "a:".repeat(64 * 1024);
        assert_eq!(parse_icon_reference(&many_components, None), None);
    }

    #[test]
    fn references_enforce_the_registry_identifier_limits() {
        let prefix = "p".repeat(MaxPrefixBytes.fixed_value() as usize);
        let name = "n".repeat(MaxNameBytes.fixed_value() as usize);
        let over_prefix = format!("{prefix}p");
        let over_name = format!("{name}n");
        for reference in [
            format!("{prefix}:{name}"),
            format!("{prefix}-{name}"),
            format!("@{over_prefix}:{prefix}:{name}"),
            format!("@{over_prefix}:{prefix}-{name}"),
            format!("{over_prefix}:{prefix}:{name}"),
        ] {
            assert_eq!(
                parse_icon_reference(&reference, Some("other")),
                Some((prefix.as_str(), name.as_str()))
            );
        }
        assert_eq!(
            parse_icon_reference(&name, Some(&prefix)),
            Some((prefix.as_str(), name.as_str()))
        );
        for reference in [
            format!("{over_prefix}:short"),
            format!("{over_prefix}-short"),
            format!("p:{over_name}"),
            format!("p-{over_name}"),
            format!("@provider:p:{over_name}"),
            format!("@provider:{over_prefix}-short"),
            over_name,
        ] {
            assert_eq!(parse_icon_reference(&reference, Some("fallback")), None);
        }
        assert_eq!(parse_icon_reference("short", Some(&over_prefix)), None);
    }
}
