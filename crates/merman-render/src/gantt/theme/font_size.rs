use crate::mermaid_style::{
    CssFontSizeContext, css_font_size_value_override, parse_style_declaration,
};

/// A presentation attribute keeps its source spelling independently of its used size.
#[derive(Debug, Clone)]
pub(crate) struct GanttFontSize {
    css: String,
    used_px: Option<f64>,
}

impl GanttFontSize {
    pub(super) fn resolve(value: Option<&serde_json::Value>, inherited_px: f64) -> Self {
        let css = match value {
            Some(serde_json::Value::String(raw)) => raw.clone(),
            Some(serde_json::Value::Number(number)) => number
                .as_f64()
                .map(|size| crate::number_format::canonical_number(size).to_string())
                .unwrap_or_else(|| "11".to_owned()),
            _ => "11".to_owned(),
        };
        let declaration = format!("font-size:{css}");
        let used_px = match parse_style_declaration(&declaration) {
            // Priority and trailing semicolons belong to declarations, not SVG attributes.
            Some(parsed) if parsed.value() != css.trim() => Some(inherited_px),
            Some(parsed) if parsed.font_size_is_negative_literal() => Some(inherited_px),
            Some(parsed) if parsed.font_size_is_literal_zero() => Some(0.0),
            // The diagram root does not supply the host document's rem basis.
            Some(parsed) if parsed.analysis().depends_on_root_font_size() => None,
            Some(parsed) => parsed
                .resolve_font_size_px(CssFontSizeContext::uniform(inherited_px))
                .or_else(|| {
                    css_font_size_value_override(&css)
                        .is_none()
                        .then_some(inherited_px)
                }),
            None => css_font_size_value_override(&css)
                .is_none()
                .then_some(inherited_px),
        };
        Self { css, used_px }
    }

    pub(crate) fn css(&self) -> &str {
        &self.css
    }

    pub(crate) fn used_px(&self) -> Option<f64> {
        self.used_px
    }

    pub(crate) fn measurement_px(&self) -> f64 {
        // Host-dependent expressions retain their CSS and cannot prove hidden text.
        self.used_px.unwrap_or(11.0)
    }
}

#[cfg(test)]
mod tests {
    use super::GanttFontSize;

    #[test]
    fn presentation_attribute_grammar_does_not_consume_declaration_priority() {
        for raw in ["0px!important", "0px;", "-1", "-1em", "invalid"] {
            for inherited in [0.0, 24.0] {
                let font = GanttFontSize::resolve(Some(&serde_json::json!(raw)), inherited);
                assert_eq!(font.css(), raw);
                assert_eq!(font.used_px(), Some(inherited), "{raw}");
            }
        }
    }

    #[test]
    fn absolute_and_relative_sizes_keep_their_attribute_spelling() {
        for (raw, inherited, used) in [
            ("24pt", 0.0, 32.0),
            ("2em", 24.0, 48.0),
            ("2em", 0.0, 0.0),
            ("50%", 24.0, 12.0),
            ("0cap", 24.0, 0.0),
            (".5px", 0.0, 0.5),
        ] {
            let font = GanttFontSize::resolve(Some(&serde_json::json!(raw)), inherited);
            assert_eq!(font.css(), raw);
            assert_eq!(font.used_px(), Some(used), "{raw}");
        }
        for raw in ["1rem", "calc(10px + 1em)"] {
            let font = GanttFontSize::resolve(Some(&serde_json::json!(raw)), 0.0);
            assert_eq!(font.used_px(), None, "{raw}");
        }
    }
}
