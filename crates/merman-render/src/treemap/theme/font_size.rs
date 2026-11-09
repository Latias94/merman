/// The inline CSSOM value read by D3, separate from the used size measured by the host.
#[derive(Debug, Clone, Copy, Default)]
pub(super) enum TreemapInlineFontSize {
    #[default]
    Generated,
    Authored {
        integer: Option<f64>,
        number: Option<f64>,
    },
    Unverified,
}

impl TreemapInlineFontSize {
    pub(super) fn authored(declaration: &crate::mermaid_style::ParsedStyleDeclaration<'_>) -> Self {
        // Callers have already admitted a measurable scalar or a CSS-wide keyword. CSSOM
        // normalizes a numeric token (e.g. `.5px` to `0.5px`) but preserves its unit.
        match parse_float_prefix(declaration.value()) {
            Some(number) => {
                cssom_numeric_prefixes(number).map_or(Self::Unverified, |(integer, number)| {
                    Self::Authored {
                        integer: Some(integer),
                        number: Some(number),
                    }
                })
            }
            None => Self::Authored {
                integer: None,
                number: None,
            },
        }
    }

    pub(super) fn integer(self, generated_px: f64) -> Option<f64> {
        match self {
            Self::Generated => Some(generated_px.trunc()),
            Self::Authored { integer, .. } => integer,
            Self::Unverified => None,
        }
    }

    pub(super) fn number(self, generated_px: f64) -> Option<f64> {
        match self {
            Self::Generated => Some(generated_px),
            Self::Authored { number, .. } => number,
            Self::Unverified => None,
        }
    }

    pub(super) fn accepts_px_assignment(self) -> bool {
        match self {
            // Keep the writer's established canonical serialization when the fitting pass
            // assigns a valid pixel value, including one equal to the generated size.
            Self::Generated => true,
            // Mermaid appends !important to every admitted authored font-size. Even an
            // equal pixel value changes CSSOM when setProperty removes that priority.
            Self::Authored { .. } => true,
            Self::Unverified => false,
        }
    }
}

fn cssom_numeric_prefixes(number: f64) -> Option<(f64, f64)> {
    // Chromium CSSNumericLiteralValue::FormatNumber uses %.6g, with ties to even.
    // Rust's scientific formatter supplies its six rounded significant digits. Only
    // the numeric prefixes are needed: %g uses fixed notation for -4 <= exponent < 6,
    // and scientific notation elsewhere. This is not Number.toString's notation.
    // CSSNumericLiteralValue::Serialize also emits integers through 999999 directly,
    // which gives the same prefixes here. Other CSSOM types and nonfinite values
    // are outside this already-admitted scalar path.
    // https://chromium.googlesource.com/chromium/src/+/494419358caf690316f160a1f27d9e771a14c033/third_party/blink/renderer/core/css/css_numeric_literal_value.cc
    let scientific = format!("{number:.5e}");
    let (_, exponent) = scientific.split_once('e')?;
    let exponent = exponent.parse::<i32>().ok()?;
    let number = parse_float_prefix(&scientific)?;
    let integer = if (-4..6).contains(&exponent) {
        number.trunc()
    } else {
        parse_int_prefix(&scientific)?
    };
    Some((integer, number))
}

fn parse_int_prefix(text: &str) -> Option<f64> {
    let text = crate::text::trim_start_ecmascript_whitespace(text);
    let sign = usize::from(matches!(text.as_bytes().first(), Some(b'+' | b'-')));
    let digits = text[sign..].bytes().take_while(u8::is_ascii_digit).count();
    (digits != 0)
        .then(|| text[..sign + digits].parse::<f64>().ok())
        .flatten()
        .filter(|number| number.is_finite())
}

fn parse_float_prefix(text: &str) -> Option<f64> {
    let text = crate::text::trim_start_ecmascript_whitespace(text);
    let bytes = text.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let integer_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let mut has_digits = end > integer_start;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let fraction_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        has_digits |= end > fraction_start;
    }
    if !has_digits {
        return None;
    }
    let mut number_end = end;
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        end += 1;
        if matches!(bytes.get(end), Some(b'+' | b'-')) {
            end += 1;
        }
        let exponent_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end > exponent_start {
            number_end = end;
        }
    }
    text[..number_end]
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_numbers_follow_decimal_prefixes_instead_of_css_unit_conversion() {
        for (text, integer, number) in [
            ("24pt", Some(24.0), Some(24.0)),
            ("2em", Some(2.0), Some(2.0)),
            ("0.5px", Some(0.0), Some(0.5)),
            (".5px", None, Some(0.5)),
            ("1e2px", Some(1.0), Some(100.0)),
            ("1em", Some(1.0), Some(1.0)),
            ("+12.5pt", Some(12.0), Some(12.5)),
            ("\u{feff}24px", Some(24.0), Some(24.0)),
            ("inherit", None, None),
            ("initial", None, None),
            ("medium", None, None),
        ] {
            assert_eq!(parse_int_prefix(text), integer, "{text}");
            assert_eq!(parse_float_prefix(text), number, "{text}");
        }
        let declaration = crate::mermaid_style::parse_style_declaration("font-size:.5px").unwrap();
        let inline = TreemapInlineFontSize::authored(&declaration);
        assert_eq!(inline.integer(38.0), Some(0.0));
        assert_eq!(inline.number(38.0), Some(0.5));
    }

    #[test]
    fn cssom_notation_uses_chromium_six_digit_general_format_before_parse_int() {
        for (source, integer, number) in [
            ("0.000001px", 1.0, 1e-6),
            ("0.0000001px", 1.0, 1e-7),
            ("1e2px", 100.0, 100.0),
            ("1e6px", 1.0, 1e6),
            ("1e12px", 1.0, 1e12),
            ("1e20px", 1.0, 1e20),
            ("100000000000000000000px", 1.0, 1e20),
            ("0.0001px", 0.0, 1e-4),
            ("999999px", 999999.0, 999999.0),
            ("999999.5px", 1.0, 1e6),
            ("123456.5px", 123456.0, 123456.0),
            ("123457.5px", 123458.0, 123458.0),
        ] {
            // Keep the declaration's borrowed source alive through the scalar preparation.
            let raw = format!("font-size:{source}");
            let declaration = crate::mermaid_style::parse_style_declaration(&raw).unwrap();
            let inline = TreemapInlineFontSize::authored(&declaration);
            assert_eq!(inline.integer(38.0), Some(integer), "{source}");
            assert_eq!(inline.number(38.0), Some(number), "{source}");
        }
    }
}
