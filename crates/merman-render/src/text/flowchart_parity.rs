//! Flowchart-specific HTML label measurement parity helpers.

pub fn flowchart_html_line_height_px(font_size_px: f64) -> f64 {
    (font_size_px.max(1.0) * 1.5).max(1.0)
}

pub fn flowchart_html_has_inline_style_tags(lower_html: &str) -> bool {
    // Detect Mermaid HTML inline styling tags in a way that avoids false positives like
    // `<br>` matching `<b`.
    //
    // We keep this intentionally lightweight (no full HTML parser); for our purposes we only
    // need to decide whether the label needs the special inline-style measurement path.
    let bytes = lower_html.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        i += 1;
        if i >= bytes.len() {
            break;
        }
        if bytes[i] == b'!' || bytes[i] == b'?' {
            continue;
        }
        if bytes[i] == b'/' {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        if start == i {
            continue;
        }
        let name = &lower_html[start..i];
        if ["font-weight", "font-style", "font-family", "font-size"]
            .iter()
            .any(|property| super::xhtml_font_default(name, property).is_some())
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_html_font_defaults_select_the_structured_measurement_path() {
        for tag in [
            "strong", "b", "em", "i", "cite", "dfn", "var", "code", "kbd", "samp", "tt", "big",
            "small",
        ] {
            assert!(
                flowchart_html_has_inline_style_tags(&format!("<{tag}>Label</{tag}>")),
                "{tag}"
            );
        }
        for html in [
            "<br/>",
            "<blockquote>Label</blockquote>",
            "<span>Label</span>",
        ] {
            assert!(!flowchart_html_has_inline_style_tags(html), "{html}");
        }
    }
}
