use serde_json::Value;

use crate::mermaid_style::{
    CssFontSizeContext, CssFontSizeOwnership, css_font_size_value_override, parse_style_declaration,
};

/// The element stylesheet uses CSSOM values, independently of legacy boundary/message arithmetic.
#[derive(Debug)]
pub(super) struct C4ElementFontSize {
    pub(super) css: Option<String>,
    pub(super) used_px: Option<f64>,
    pub(super) inherits_root: bool,
}

impl C4ElementFontSize {
    pub(super) fn resolve(value: Option<&Value>, root_px: Option<f64>) -> Self {
        let inherited = || Self {
            css: None,
            used_px: root_px,
            inherits_root: true,
        };
        // The initialized per-type default is 14px. Explicit falsy/rejected values instead
        // remove the CSSOM property and expose the inherited root; they do not restore 14px.
        let Some(value) = value else {
            return Self {
                css: Some("14px".to_owned()),
                used_px: Some(14.0),
                inherits_root: false,
            };
        };
        if !crate::config::json_value_is_truthy(value) {
            return inherited();
        }
        let css = match value {
            Value::Number(number) => match number.as_f64() {
                Some(number) => format!("{number}px"),
                None => return inherited(),
            },
            Value::String(raw) => raw.trim().to_owned(),
            _ => return inherited(),
        };
        let Some(ownership) = css_font_size_value_override(&css) else {
            return inherited();
        };
        let declaration = format!("font-size:{css}");
        let Some(parsed) = parse_style_declaration(&declaration) else {
            // Unsafe/unmeasurable host syntax cannot authorize inherited theme evidence.
            return Self {
                css: None,
                used_px: None,
                inherits_root: false,
            };
        };
        // CSSOM setProperty receives a value, without a priority argument or a declaration list.
        if !super::super::config::cssom_accepts_value_priority(
            parsed.value(),
            ownership.important(),
        ) || parsed.source_value() != css
        {
            return inherited();
        }
        let inherits_root = ownership.ownership() == CssFontSizeOwnership::Inherited;
        let used_px = if parsed.font_size_is_literal_zero() {
            Some(0.0)
        } else if inherits_root {
            root_px
        } else if ownership.ownership() == CssFontSizeOwnership::SourceOwned {
            parsed.analysis().context_free_font_size_px().or_else(|| {
                root_px
                    .and_then(|root| parsed.resolve_font_size_px(CssFontSizeContext::uniform(root)))
            })
        } else {
            None
        };
        let used_px = used_px.filter(|size| *size == 0.0 || (*size as f32).is_normal());
        Self {
            css: Some(css),
            used_px,
            inherits_root,
        }
    }
}
