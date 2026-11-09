//! Numeric value conversion shared by source and output consumers.

/// Converts unsigned base-2, base-8, or base-16 digits with one binary64 rounding step.
///
/// The caller owns prefixes, signs, and token boundaries. Empty or invalid digits are rejected;
/// values exceeding binary64 range become positive infinity.
pub fn parse_power_of_two_digits(digits: &str, radix: u32) -> Option<f64> {
    if !matches!(radix, 2 | 8 | 16)
        || digits.is_empty()
        || !digits.chars().all(|digit| digit.is_digit(radix))
    {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(0.0);
    }
    let bits_per_digit = radix.ilog2() as usize;
    let leading_len = digits.len().min(64 / bits_per_digit);
    let leading = u64::from_str_radix(&digits[..leading_len], radix).ok()?;
    let trailing = &digits[leading_len..];
    // A sticky bit retains whether discarded digits cross a rounding midpoint.
    let sticky = u64::from(trailing.bytes().any(|digit| digit != b'0'));
    let shift = trailing.len().saturating_mul(bits_per_digit);
    Some(if shift >= 1024 {
        f64::INFINITY
    } else {
        ((leading | sticky) as f64) * 2_f64.powi(shift as i32)
    })
}
