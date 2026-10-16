pub(super) fn normalize_xhtml_fragment_for_foreign_object(raw: &str) -> String {
    let config = merman_core::MermaidConfig::from_value(serde_json::json!({
        "securityLevel": "strict"
    }));
    merman_core::sanitize::sanitize_xhtml_in_svg_context(raw, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_xhtml_fragment_splits_root_svg_anchor_from_html_children() {
        assert_eq!(
            normalize_xhtml_fragment_for_foreign_object(
                r#"<a href='https://example.com'><code>code</code></a>"#,
            ),
            r#"<a xmlns="http://www.w3.org/2000/svg" href="https://example.com"></a><code>code</code>"#,
        );
    }

    #[test]
    fn normalize_xhtml_fragment_preserves_anchor_inside_html_context() {
        assert_eq!(
            normalize_xhtml_fragment_for_foreign_object(
                r#"<p><a href='https://example.com'><code>code</code></a></p>"#,
            ),
            r#"<p><a href="https://example.com"><code>code</code></a></p>"#,
        );
    }

    #[test]
    fn normalize_xhtml_fragment_sanitizes_before_svg_context_recovery() {
        assert_eq!(
            normalize_xhtml_fragment_for_foreign_object(r#"<g>x<b>y</b>z</g>"#),
            "",
        );
    }

    #[test]
    fn normalize_xhtml_fragment_handles_deep_nested_html_with_small_stack() {
        const DEPTH: usize = 2_048;
        let handle = std::thread::Builder::new()
            .name("architecture-deep-xhtml-fragment".to_string())
            .stack_size(if cfg!(debug_assertions) {
                512 * 1024
            } else {
                64 * 1024
            })
            .spawn(|| {
                let mut raw = String::new();
                for _ in 0..DEPTH {
                    raw.push_str("<span>");
                }
                raw.push_str("Icon");
                for _ in 0..DEPTH {
                    raw.push_str("</span>");
                }
                normalize_xhtml_fragment_for_foreign_object(&raw)
            })
            .expect("spawn deep XHTML fragment test");

        let normalized = handle
            .join()
            .expect("deep XHTML fragment normalization should not overflow");
        assert!(normalized.contains("Icon"));
    }
}
