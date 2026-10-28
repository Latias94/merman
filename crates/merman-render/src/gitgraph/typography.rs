use crate::text::{TextStyle, apply_css_font_declarations};

/// Final label classes bind typography before Mermaid measures their bbox.
pub(crate) struct GitGraphLabelTypography {
    pub(crate) style: TextStyle,
    font_weight_css: Option<String>,
}

impl GitGraphLabelTypography {
    pub(crate) fn resolve(
        font_family: &str,
        font_size_px: f64,
        font_weight_css: Option<&str>,
    ) -> Self {
        let mut style = TextStyle {
            font_family: Some(font_family.to_owned()),
            font_size: font_size_px,
            font_weight: None,
            font_style: None,
        };
        if let Some(weight) = font_weight_css {
            let parsed = apply_css_font_declarations(&style, &format!("font-weight:{weight}"));
            style.font_weight = parsed.font_weight.or_else(|| {
                // Unknown CSS remains available to the host instead of becoming normal.
                (!crate::text::css_font_value_inherits(weight)).then(|| weight.to_owned())
            });
        }
        Self {
            style,
            font_weight_css: font_weight_css.map(str::to_owned),
        }
    }

    pub(crate) fn font_weight_css(&self) -> Option<&str> {
        self.font_weight_css.as_deref()
    }
}
