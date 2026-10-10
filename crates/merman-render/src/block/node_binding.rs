use std::fmt::Write as _;
fn push_ordered_decl(out: &mut Vec<(String, String)>, key: &str, raw: &str) {
    if let Some((_, value)) = out.iter_mut().find(|(existing, _)| existing == key) {
        *value = raw.to_string();
        return;
    }
    out.push((key.to_string(), raw.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_declarations_keep_first_position_and_last_value() {
        let binding = lower(
            &[
                "fill:red".into(),
                "stroke:var(--edge)".into(),
                "fill:blue !important".into(),
                "color:var(--text)".into(),
            ],
            None,
            None,
            None,
        );
        assert_eq!(
            binding.box_style,
            "fill:blue !important;stroke:var(--edge);"
        );
        assert_eq!(binding.text_style, "color:var(--text);");
        assert_eq!(binding.svg_text_style, "fill:var(--text);");
        assert_eq!(binding.div_style_prefix, "color: var(--text); ");
        assert!(binding.html_paragraph_color.is_none());
    }

    #[test]
    fn prepared_typed_candidates_reach_their_separate_label_sinks() {
        let binding = lower(
            &["stroke-width:2px".into()],
            Some("#123456"),
            Some("transparent"),
            Some("#abcdef"),
        );
        assert_eq!(
            binding.box_style,
            "stroke-width:2px;fill:#123456;stroke:transparent;"
        );
        assert_eq!(binding.svg_text_style, "fill:#abcdef;");
        assert_eq!(binding.html_paragraph_color.as_deref(), Some("#abcdef"));
        assert_eq!(binding.text_style, "");
    }
}

#[derive(Debug)]
pub(crate) struct BlockNodeTerminalBinding {
    pub(crate) box_style: String,
    pub(crate) text_style: String,
    pub(crate) svg_text_style: String,
    pub(crate) div_style_prefix: String,
    pub(crate) html_paragraph_color: Option<String>,
}

pub(crate) fn lower(
    styles: &[String],
    fill: Option<&str>,
    stroke: Option<&str>,
    label: Option<&str>,
) -> BlockNodeTerminalBinding {
    let mut box_decls: Vec<(String, String)> = Vec::new();
    let mut text_decls: Vec<(String, String)> = Vec::new();

    for raw in styles {
        let trimmed = raw.trim().trim_end_matches(';').trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((key, value)) = crate::mermaid_style::parse_safe_style_decl(trimmed) else {
            continue;
        };
        if matches!(
            key,
            "fill"
                | "stroke"
                | "stroke-width"
                | "stroke-dasharray"
                | "opacity"
                | "fill-opacity"
                | "stroke-opacity"
        ) {
            push_ordered_decl(&mut box_decls, key, trimmed);
        }
        if crate::mermaid_style::is_label_style_key(key) {
            let _ = value;
            push_ordered_decl(&mut text_decls, key, trimmed);
        }
    }

    let style_attr = |decls: &[(String, String)]| -> String {
        let mut out = String::new();
        for (_, raw) in decls {
            out.push_str(raw);
            out.push(';');
        }
        out
    };

    let mut div_prefix = String::new();
    let mut svg_text_style = String::new();
    for (key, raw) in &text_decls {
        if key == "color" {
            let value = raw.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
            let _ = write!(&mut svg_text_style, "fill:{value};");
            if !value.is_empty() {
                let _ = write!(
                    &mut div_prefix,
                    "color: {}; ",
                    crate::svg::cssom_color_value(value)
                );
            }
        } else {
            svg_text_style.push_str(raw);
            svg_text_style.push(';');
            div_prefix.push_str(raw);
            div_prefix.push_str("; ");
        }
    }

    let mut box_style = style_attr(&box_decls);
    for (key, value) in [("fill", fill), ("stroke", stroke)] {
        if let Some(value) = value {
            let _ = write!(box_style, "{key}:{value};");
        }
    }
    if let Some(value) = label {
        let _ = write!(svg_text_style, "fill:{value};");
    }
    BlockNodeTerminalBinding {
        html_paragraph_color: label.map(str::to_owned),
        box_style,
        text_style: style_attr(&text_decls),
        svg_text_style,
        div_style_prefix: div_prefix,
    }
}
