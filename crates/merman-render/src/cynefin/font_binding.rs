use serde_json::Value;

use super::CynefinTextRole;
use crate::mermaid_style::{
    css_font_size_is_zero, css_font_size_value_override, is_resource_free_css_value,
    resolve_explicit_css_absolute_font_size_px,
};

/// The four expressions in Cynefin's stylesheet keep their JSON operand types until binding.
#[derive(Debug, Clone)]
pub(crate) struct CynefinFontBinding {
    domain: CynefinFontSize,
    item: CynefinFontSize,
    annotation: CynefinFontSize,
    title: CynefinFontSize,
    inherited_px: f64,
}

#[derive(Debug, Clone)]
struct CynefinFontSize {
    css: String,
    used_px: Option<f64>,
}

impl CynefinFontSize {
    fn new(css: Option<String>, inherited_px: f64) -> Self {
        let Some(css) = css.filter(|css| is_resource_free_css_value(css)) else {
            // Unadmitted source is never interpolated into the stylesheet or certified as
            // applied. The safe invalid token preserves best-effort inherited rendering.
            return Self {
                css: "NaNpx".to_owned(),
                used_px: None,
            };
        };
        let used_px = if css_font_size_value_override(&css).is_none() {
            // Appending `px` makes strings such as `24pt` and `potato` invalid CSS. They
            // inherit; they are not interpreted as CSS lengths or replaced with role defaults.
            (inherited_px == 0.0 || (inherited_px > 0.0 && (inherited_px as f32).is_normal()))
                .then_some(inherited_px)
        } else {
            resolve_explicit_css_absolute_font_size_px(&css)
                .or_else(|| css_font_size_is_zero(&css).then_some(0.0))
                .and_then(|size| admit_painter_font_size_px(&css, size))
        };
        Self { css, used_px }
    }
}

/// Retain zero, but certify positive text only inside binary32's normal range for both the
/// authored CSS coefficient and the final pixel value. Subnormal/overflowed values stay unknown.
fn admit_painter_font_size_px(css: &str, size: f64) -> Option<f64> {
    if size == 0.0 {
        return Some(0.0);
    }
    if size <= 0.0 || !(size as f32).is_normal() {
        return None;
    }
    let mut parser = cssparser::Parser::new(css);
    matches!(parser.next(), Ok(cssparser::Token::Dimension { value, .. })
        if *value > 0.0 && value.is_normal())
    .then_some(size)
}

impl CynefinFontBinding {
    pub(super) fn new(config: &Value) -> Self {
        // Match the already prepared common root rule, including its number canonicalization.
        let inherited_px = crate::number_format::canonicalize_number(
            crate::config::config_theme_font_size_css_or_root_number_px(config, 16.0),
        );
        let values = &config["themeVariables"]["cynefin"];
        let domain = FontOperand::new(values.get("domainFontSize"), 16.0);
        let item = FontOperand::new(values.get("itemFontSize"), 12.0);
        Self {
            domain: CynefinFontSize::new(domain.interpolate_px(), inherited_px),
            item: CynefinFontSize::new(item.interpolate_px(), inherited_px),
            annotation: CynefinFontSize::new(item.subtract_one_px(), inherited_px),
            title: CynefinFontSize::new(domain.add_two_px(), inherited_px),
            inherited_px,
        }
    }

    fn role(&self, role: CynefinTextRole) -> &CynefinFontSize {
        match role {
            CynefinTextRole::DomainLabel => &self.domain,
            CynefinTextRole::Item => &self.item,
            CynefinTextRole::Subtitle | CynefinTextRole::TransitionLabel => &self.annotation,
            CynefinTextRole::Title => &self.title,
        }
    }

    pub(crate) fn css(&self, role: CynefinTextRole) -> &str {
        &self.role(role).css
    }

    pub(crate) fn used_px(&self, role: CynefinTextRole) -> Option<f64> {
        self.role(role).used_px
    }

    pub(crate) fn measurement_px(&self, role: CynefinTextRole) -> f64 {
        self.used_px(role).unwrap_or(self.inherited_px)
    }
}

enum FontOperand<'a> {
    Number(f64),
    String(&'a str),
    Unsupported,
}

impl<'a> FontOperand<'a> {
    fn new(value: Option<&'a Value>, fallback: f64) -> Self {
        match value {
            None => Self::Number(fallback),
            Some(Value::Number(number)) => number
                .as_f64()
                .map(Self::Number)
                .unwrap_or(Self::Unsupported),
            Some(Value::String(raw)) => Self::String(raw),
            Some(_) => Self::Unsupported,
        }
    }

    fn interpolate_px(&self) -> Option<String> {
        match self {
            Self::Number(number) => Some(number_px(*number)),
            Self::String(raw) => Some(format!("{raw}px")),
            Self::Unsupported => None,
        }
    }

    fn subtract_one_px(&self) -> Option<String> {
        let number = match self {
            Self::Number(number) => *number,
            Self::String(raw) => subtraction_operand(raw)?,
            Self::Unsupported => return None,
        };
        Some(number_px(number - 1.0))
    }

    fn add_two_px(&self) -> Option<String> {
        match self {
            Self::Number(number) => Some(number_px(number + 2.0)),
            Self::String(raw) => Some(format!("{raw}2px")),
            Self::Unsupported => None,
        }
    }
}

fn number_px(number: f64) -> String {
    let mut buffer = ryu_js::Buffer::new();
    format!(
        "{}px",
        buffer.format(if number == 0.0 { 0.0 } else { number })
    )
}

/// Only the stylesheet's numeric subtraction needs a string-to-number conversion.
/// Decimal strings use binary64 parsing; bounded radix integers retain their exact conversion.
/// Unsupported integer magnitudes stay unverified rather than growing a JavaScript evaluator.
fn subtraction_operand(raw: &str) -> Option<f64> {
    let raw = crate::text::trim_ecmascript_whitespace(raw);
    if raw.is_empty() {
        return Some(0.0);
    }
    for (prefixes, radix) in [(["0x", "0X"], 16), (["0o", "0O"], 8), (["0b", "0B"], 2)] {
        if let Some(digits) = prefixes.iter().find_map(|prefix| raw.strip_prefix(*prefix)) {
            if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
                return Some(f64::NAN);
            }
            return u64::from_str_radix(digits, radix)
                .ok()
                .map(|value| value as f64);
        }
    }
    if matches!(raw, "Infinity" | "+Infinity") {
        return Some(f64::INFINITY);
    }
    if raw == "-Infinity" {
        return Some(f64::NEG_INFINITY);
    }
    if !raw
        .bytes()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return Some(f64::NAN);
    }
    Some(raw.parse().unwrap_or(f64::NAN))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numeric_and_string_operands_keep_distinct_stylesheet_expressions() {
        for (value, direct, annotation, title, item_px, title_px) in [
            (json!(24), "24px", "23px", "26px", 24.0, 26.0),
            (json!("24"), "24px", "23px", "242px", 24.0, 242.0),
            (json!("24pt"), "24ptpx", "NaNpx", "24pt2px", 32.0, 32.0),
            (
                json!("potato"),
                "potatopx",
                "NaNpx",
                "potato2px",
                32.0,
                32.0,
            ),
        ] {
            let binding = CynefinFontBinding::new(&json!({"themeVariables": {
                "fontSize": "24pt", "cynefin": {"domainFontSize": value, "itemFontSize": value}
            }}));
            assert_eq!(binding.css(CynefinTextRole::DomainLabel), direct);
            assert_eq!(binding.css(CynefinTextRole::Item), direct);
            assert_eq!(binding.css(CynefinTextRole::Subtitle), annotation);
            assert_eq!(binding.css(CynefinTextRole::Title), title);
            assert_eq!(binding.used_px(CynefinTextRole::Item), Some(item_px));
            assert_eq!(binding.used_px(CynefinTextRole::Title), Some(title_px));
        }
    }

    #[test]
    fn unsafe_and_unrepresentable_final_sizes_remain_unverified() {
        for value in [
            "1e-50",
            "1e-39",
            "1e-400",
            "1e400",
            "1px;fill:red",
            "var(--size)",
        ] {
            let binding = CynefinFontBinding::new(&json!({"themeVariables": {"cynefin": {
                "domainFontSize": value, "itemFontSize": value
            }}}));
            assert_eq!(binding.used_px(CynefinTextRole::Item), None, "{value}");
            assert_eq!(binding.used_px(CynefinTextRole::Title), None, "{value}");
            assert!(is_resource_free_css_value(
                binding.css(CynefinTextRole::Item)
            ));
        }
    }

    #[test]
    fn painter_admission_checks_token_and_converted_representation_without_an_epsilon() {
        let minimum_normal = f64::from(f32::MIN_POSITIVE);
        let css = number_px(minimum_normal);
        assert_eq!(
            admit_painter_font_size_px(&css, minimum_normal),
            Some(minimum_normal)
        );
        let largest_subnormal = f64::from(f32::from_bits(f32::MIN_POSITIVE.to_bits() - 1));
        assert_eq!(
            admit_painter_font_size_px(&number_px(largest_subnormal), largest_subnormal),
            None
        );
        assert_eq!(admit_painter_font_size_px("0px", 0.0), Some(0.0));
        assert_eq!(admit_painter_font_size_px("24pt", 32.0), Some(32.0));
        assert_eq!(admit_painter_font_size_px("1e-39in", 1e-39 * 96.0), None);
        assert_eq!(
            admit_painter_font_size_px("1.2e-38q", 1.2e-38 * 96.0 / 101.6),
            None
        );
    }

    #[test]
    fn subtraction_is_full_number_conversion_not_a_css_or_integer_prefix() {
        for (raw, expected) in [
            ("24", 24.0),
            ("24.5", 24.5),
            ("\u{feff} +24.5 ", 24.5),
            ("0x18", 24.0),
            ("0o30", 24.0),
            ("0b11000", 24.0),
            ("", 0.0),
        ] {
            assert_eq!(subtraction_operand(raw), Some(expected), "{raw}");
        }
        for raw in ["24pt", "potato", "24 0", "0x", "+0x18", "\u{85}24", "inf"] {
            assert!(subtraction_operand(raw).is_some_and(f64::is_nan), "{raw}");
        }
        assert_eq!(subtraction_operand("0x10000000000000000"), None);
    }
}
