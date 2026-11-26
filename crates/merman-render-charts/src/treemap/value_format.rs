use merman_render_core::{Error, Result};

fn treemap_group_decimal(value: &str) -> String {
    let (sign, unsigned) = value
        .strip_prefix('-')
        .or_else(|| value.strip_prefix('−'))
        .map_or(("", value), |rest| ("−", rest));
    let (integer, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, ""), |(integer, fraction)| (integer, fraction));
    let mut grouped = String::with_capacity(value.len() + integer.len() / 3);
    for (index, digit) in integer.bytes().enumerate() {
        if index != 0 && (integer.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(char::from(digit));
    }
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{fraction}")
    }
}

fn treemap_js_number(value: f64) -> String {
    if !value.is_finite() {
        return "NaN".to_string();
    }
    let mut buffer = ryu_js::Buffer::new();
    let rendered = buffer
        .format_finite(if value == -0.0 { 0.0 } else { value })
        .to_string();
    if let Some(rest) = rendered.strip_prefix('-') {
        format!("−{rest}")
    } else {
        rendered
    }
}

fn treemap_exponential_parts(value: f64, significant_digits: usize) -> Option<(String, i32)> {
    if !value.is_finite() || value == 0.0 {
        return None;
    }
    let raw = format!("{:.*e}", significant_digits.saturating_sub(1), value.abs());
    let (mantissa, exponent) = raw.split_once('e')?;
    let digits = mantissa.replace('.', "");
    let exponent = exponent.parse::<i32>().ok()?;
    Some((digits, exponent))
}

fn treemap_trim_decimal(mut value: String) -> String {
    if let Some(exponent_start) = value.find('e') {
        let exponent = value.split_off(exponent_start);
        let mut mantissa = treemap_trim_decimal(value);
        mantissa.push_str(&exponent);
        return mantissa;
    }
    if let Some(dot) = value.find('.') {
        while value.ends_with('0') {
            value.pop();
        }
        if value.len() == dot + 1 {
            value.pop();
        }
    }
    value
}

fn treemap_format_significant(value: f64, precision: usize) -> String {
    if !value.is_finite() {
        return "NaN".to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }

    let precision = precision.clamp(1, 21);
    let negative = value.is_sign_negative();
    let Some((mut digits, exponent)) = treemap_exponential_parts(value, precision) else {
        return treemap_js_number(value);
    };
    while digits.ends_with('0') {
        digits.pop();
    }

    let mut formatted = if exponent >= precision as i32 || exponent < -6 {
        let mantissa = if digits.len() == 1 {
            digits
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        let exponent = if exponent >= 0 {
            format!("+{exponent}")
        } else {
            exponent.to_string()
        };
        format!("{mantissa}e{exponent}")
    } else {
        let decimal_position = exponent + 1;
        if decimal_position <= 0 {
            format!("0.{}{}", "0".repeat((-decimal_position) as usize), digits)
        } else if decimal_position as usize >= digits.len() {
            format!(
                "{}{}",
                digits,
                "0".repeat(decimal_position as usize - digits.len())
            )
        } else {
            let split_at = decimal_position as usize;
            format!("{}.{}", &digits[..split_at], &digits[split_at..])
        }
    };
    formatted = treemap_trim_decimal(formatted);
    if negative {
        format!("−{formatted}")
    } else {
        formatted
    }
}

fn treemap_format_fixed(value: f64, precision: usize, grouped: bool) -> String {
    let formatted = format!("{:.*}", precision.min(20), value);
    if grouped {
        treemap_group_decimal(&formatted)
    } else if let Some(unsigned) = formatted.strip_prefix('-') {
        format!("−{unsigned}")
    } else {
        formatted
    }
}

fn treemap_format_scientific(value: f64, precision: usize) -> String {
    let raw = format!("{:.*e}", precision.min(20), value.abs());
    let Some((mantissa, exponent)) = raw.split_once('e') else {
        return raw;
    };
    let exponent = exponent
        .parse::<i32>()
        .map_or_else(|_| exponent.to_string(), |value| format!("{value:+}"));
    let formatted = format!("{mantissa}e{exponent}");
    if value.is_sign_negative() {
        format!("−{formatted}")
    } else {
        formatted
    }
}

fn treemap_format_precision(format_str: &str) -> Option<usize> {
    let dot = format_str.find('.')?;
    let digits = format_str[dot + 1..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().unwrap_or(6))
}

fn treemap_is_valid_but_unsupported_d3_format(format_str: &str) -> bool {
    let format_str = format_str.trim();
    let Some(type_char) = format_str.chars().last() else {
        return false;
    };
    if !matches!(
        type_char,
        '%' | 'b' | 'c' | 'd' | 'e' | 'f' | 'g' | 'n' | 'o' | 'p' | 'r' | 's' | 'x' | 'X'
    ) {
        return false;
    }
    let prefix = &format_str[..format_str.len() - type_char.len_utf8()];
    prefix.chars().all(|character| {
        character.is_ascii_digit()
            || matches!(
                character,
                '.' | ',' | '+' | '-' | ' ' | '(' | '$' | '#' | '<' | '>' | '=' | '^' | '~'
            )
    })
}

fn treemap_default_format(value: f64) -> String {
    let mut formatted = treemap_format_significant(value, 12);
    if !formatted.contains('e') {
        formatted = treemap_group_decimal(&formatted);
    }
    formatted
}

fn treemap_format_standard(value: f64, format_str: &str) -> Option<String> {
    let format_str = format_str.trim();
    if format_str.is_empty() {
        return Some(treemap_js_number(value));
    }

    let (grouped, body) = format_str
        .strip_prefix(',')
        .map_or((false, format_str), |rest| (true, rest));
    if body.contains(',') {
        return None;
    }

    let (precision, type_char) = if let Some(rest) = body.strip_prefix('.') {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let precision = rest[..digits].parse().unwrap_or(6);
        let suffix = &rest[digits..];
        let type_char = match suffix {
            "" => None,
            "%" | "e" | "f" | "g" => suffix.chars().next(),
            _ => return None,
        };
        (Some(precision), type_char)
    } else {
        let type_char = match body {
            "" => None,
            "%" | "e" | "f" | "g" => body.chars().next(),
            _ => return None,
        };
        (None, type_char)
    };

    if type_char.is_none() && precision.is_none() && !grouped {
        return None;
    }

    match type_char {
        Some('%') => Some(format!(
            "{}%",
            treemap_format_fixed(value * 100.0, precision.unwrap_or(6), false)
        )),
        Some('e') => Some(treemap_format_scientific(value, precision.unwrap_or(6))),
        Some('f') => Some(treemap_format_fixed(value, precision.unwrap_or(6), grouped)),
        Some('g') => Some(treemap_format_significant(value, precision.unwrap_or(6))),
        None => {
            let mut formatted = treemap_format_significant(value, precision.unwrap_or(12));
            if grouped && !formatted.contains('e') {
                formatted = treemap_group_decimal(&formatted);
            }
            Some(formatted)
        }
        Some(_) => None,
    }
}

fn treemap_format_value(value: f64, format_str: &str) -> Option<String> {
    let format_str = format_str.trim();
    let format_str = if format_str.is_empty() {
        ","
    } else {
        format_str
    };
    if format_str == "$0,0" {
        return Some(format!(
            "${}",
            treemap_format_standard(value, ",").unwrap_or_else(|| treemap_default_format(value))
        ));
    }
    if format_str.starts_with('$') && format_str.contains(',') {
        let precision = treemap_format_precision(format_str)
            .map_or_else(String::new, |precision| format!(".{precision}"));
        return Some(format!(
            "${}",
            treemap_format_standard(value, &format!(",{precision}"))
                .unwrap_or_else(|| treemap_default_format(value))
        ));
    }
    if let Some(rest) = format_str.strip_prefix('$') {
        if let Some(formatted) = treemap_format_standard(value, rest) {
            return Some(format!("${formatted}"));
        }
        return (!treemap_is_valid_but_unsupported_d3_format(rest))
            .then(|| treemap_default_format(value));
    }
    treemap_format_standard(value, format_str).or_else(|| {
        if treemap_is_valid_but_unsupported_d3_format(format_str) {
            None
        } else {
            Some(treemap_default_format(value))
        }
    })
}

pub fn format_treemap_value(value: f64, format_str: &str) -> Result<String> {
    treemap_format_value(value, format_str).ok_or_else(|| Error::InvalidModel {
        message: format!("unsupported Treemap valueFormat `{}`", format_str.trim()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treemap_value_formats_match_mermaid_special_cases() {
        assert_eq!(
            treemap_format_value(1_234_567.0, ","),
            Some("1,234,567".to_string())
        );
        assert_eq!(
            treemap_format_value(1_234.5, ""),
            Some("1,234.5".to_string())
        );
        assert_eq!(
            treemap_format_value(-1_234.5, ","),
            Some("−1,234.5".to_string())
        );
        assert_eq!(treemap_format_value(0.0, ","), Some("0".to_string()));
        assert_eq!(
            treemap_format_value(0.00000000012, ","),
            Some("1.2e-10".to_string())
        );
        assert_eq!(
            treemap_format_value(12_000_000_000.0, ".2g"),
            Some("1.2e+10".to_string())
        );
        assert_eq!(treemap_format_value(0.35, ".1%"), Some("35.0%".to_string()));
        assert_eq!(
            treemap_format_value(0.6723, ".2f"),
            Some("0.67".to_string())
        );
        assert_eq!(
            treemap_format_value(1_234_567.0, ".2e"),
            Some("1.23e+6".to_string())
        );
        assert_eq!(
            treemap_format_value(700_000.0, "$0,0"),
            Some("$700,000".to_string())
        );
        assert_eq!(
            treemap_format_value(0.35, "$.1%"),
            Some("$35.0%".to_string())
        );
        assert_eq!(
            treemap_format_value(1_234.5, "$,.2f"),
            Some("$1.2e+3".to_string())
        );
        assert_eq!(
            treemap_format_value(1_234.5, "$,.0f"),
            Some("$1e+3".to_string())
        );
        assert_eq!(
            treemap_format_value(1_234.5, "invalid"),
            Some("1,234.5".to_string())
        );
        assert_eq!(
            treemap_format_value(1_234.5, "$invalid"),
            Some("1,234.5".to_string())
        );
        assert!(treemap_format_value(1_234.5, ".2s").is_none());
        assert!(treemap_format_value(1_234.5, "d").is_none());
        assert!(treemap_format_value(1_234.5, "+.1f").is_none());
        assert_eq!(
            treemap_format_value(1_234.5, "$,.2s"),
            Some("$1.2e+3".to_string())
        );
    }
}
