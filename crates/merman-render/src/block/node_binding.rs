use std::fmt::Write as _;
fn is_box_style_key(key: &str) -> bool {
    matches!(
        key,
        "fill"
            | "stroke"
            | "stroke-width"
            | "stroke-dasharray"
            | "opacity"
            | "fill-opacity"
            | "stroke-opacity"
    )
}

fn source_style_winners(
    styles: &[String],
) -> Vec<(&str, crate::mermaid_style::ParsedStyleDeclaration<'_>)> {
    let mut winners: Vec<(&str, crate::mermaid_style::ParsedStyleDeclaration<'_>)> = Vec::new();
    for raw in styles {
        let raw = raw.trim().trim_end_matches(';').trim();
        let Some(parsed) = crate::mermaid_style::parse_style_declaration(raw) else {
            continue;
        };
        if !is_box_style_key(parsed.property())
            && !crate::mermaid_style::is_label_style_key(parsed.property())
        {
            continue;
        }
        if let Some(current) = winners
            .iter_mut()
            .find(|(_, winner)| winner.property() == parsed.property())
        {
            // Retain the first property's position and the winning source spelling. An
            // ordinary later declaration cannot replace an earlier important value.
            if parsed.important() || !current.1.important() {
                *current = (raw, parsed);
            }
        } else {
            winners.push((raw, parsed));
        }
    }
    winners
}

pub(super) fn source_font_weight(
    styles: &[String],
    inherited: &crate::text::TextStyle,
) -> Option<u16> {
    let inherited = inherited
        .font_weight
        .as_deref()
        .and_then(|value| crate::text::resolve_css_font_weight(value, 400))
        .unwrap_or(400);
    source_style_winners(styles)
        .into_iter()
        .find(|(_, parsed)| parsed.property() == "font-weight")
        .and_then(|(_, parsed)| crate::text::resolve_css_font_weight(parsed.value(), inherited))
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
    fn source_declaration_priority_keeps_important_winner_and_original_position() {
        for (last, expected) in [
            ("FONT-WEIGHT:normal", "font-weight:bold !important"),
            (
                "FONT-WEIGHT:normal !important",
                "FONT-WEIGHT:normal !important",
            ),
        ] {
            let binding = lower(
                &[
                    "font-weight:bold !important".into(),
                    "color:#123456".into(),
                    last.into(),
                    "fill:#abcdef !important".into(),
                    "FILL:#fedcba".into(),
                ],
                None,
                None,
                None,
            );
            assert_eq!(binding.text_style, format!("{expected};color:#123456;"));
            assert_eq!(binding.svg_text_style, format!("{expected};fill:#123456;"));
            assert_eq!(
                binding.div_style_prefix,
                format!("{expected}; color: rgb(18, 52, 86); ")
            );
            assert_eq!(binding.box_style, "fill:#abcdef !important;");
        }
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
    let mut box_style = String::new();
    let mut text_style = String::new();
    let mut div_prefix = String::new();
    let mut svg_text_style = String::new();
    for (raw, parsed) in source_style_winners(styles) {
        let key = parsed.property();
        if is_box_style_key(key) {
            box_style.push_str(raw);
            box_style.push(';');
        }
        if !crate::mermaid_style::is_label_style_key(key) {
            continue;
        }
        text_style.push_str(raw);
        text_style.push(';');
        if key == "color" {
            let value = parsed.source_value();
            let _ = write!(&mut svg_text_style, "fill:{value};");
            let _ = write!(
                &mut div_prefix,
                "color: {}; ",
                crate::svg::cssom_color_value(value)
            );
        } else {
            svg_text_style.push_str(raw);
            svg_text_style.push(';');
            div_prefix.push_str(raw);
            div_prefix.push_str("; ");
        }
    }

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
        text_style,
        svg_text_style,
        div_style_prefix: div_prefix,
    }
}
