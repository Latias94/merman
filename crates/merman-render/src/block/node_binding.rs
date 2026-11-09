use std::fmt::Write as _;

pub(super) fn is_font_key(key: &str) -> bool {
    matches!(
        key,
        "font-family" | "font-size" | "font-weight" | "font-style"
    )
}
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

pub(super) fn source_style_winners(
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

/// Mermaid compiles assigned classes in assignment order, followed by direct styles.
/// The compiled label declarations are inline important values, above class stylesheet rules.
pub(super) fn source_font_styles(
    styles: &[String],
    classes: &[String],
    definitions: &indexmap::IndexMap<String, merman_model::families::block::BlockClassDef>,
) -> Vec<String> {
    let mut fonts = indexmap::IndexMap::new();
    for class in classes {
        if let Some(definition) = definitions.get(class) {
            for raw in &definition.styles {
                if let Some(declaration) = crate::mermaid_style::parse_style_declaration(raw)
                    && is_font_key(declaration.property())
                {
                    fonts.insert(
                        declaration.property().to_owned(),
                        declaration.value().to_owned(),
                    );
                }
            }
        }
    }
    let compiled = !fonts.is_empty();
    let mut direct = Vec::new();
    for (raw, declaration) in source_style_winners(styles) {
        if is_font_key(declaration.property()) {
            if compiled {
                fonts.insert(
                    declaration.property().to_owned(),
                    declaration.value().to_owned(),
                );
            } else {
                direct.push(if declaration.important() {
                    raw.to_owned()
                } else {
                    format!("{raw} !important")
                });
            }
        }
    }
    if compiled {
        fonts
            .into_iter()
            .map(|(key, value)| format!("{key}:{value} !important"))
            .collect()
    } else {
        direct
    }
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
            &[],
            &indexmap::IndexMap::new(),
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
                &[],
                &indexmap::IndexMap::new(),
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
            &[],
            &indexmap::IndexMap::new(),
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
    classes: &[String],
    definitions: &indexmap::IndexMap<String, merman_model::families::block::BlockClassDef>,
    fill: Option<&str>,
    stroke: Option<&str>,
    label: Option<&str>,
) -> BlockNodeTerminalBinding {
    let mut box_style = String::new();
    let mut text_style = String::new();
    let mut div_prefix = String::new();
    let mut svg_text_style = String::new();
    let fonts = source_font_styles(styles, classes, definitions);
    let mut declarations = source_style_winners(styles)
        .into_iter()
        .map(|(raw, _)| raw.to_owned())
        .collect::<Vec<_>>();
    for font in fonts {
        let parsed = crate::mermaid_style::parse_style_declaration(&font)
            .expect("prepared font declaration");
        if let Some(raw) = declarations.iter_mut().find(|raw| {
            crate::mermaid_style::parse_style_declaration(raw)
                .is_some_and(|declaration| declaration.property() == parsed.property())
        }) {
            *raw = font;
        } else {
            declarations.push(font);
        }
    }
    for raw in &declarations {
        let parsed = crate::mermaid_style::parse_style_declaration(raw)
            .expect("prepared source declaration");
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
