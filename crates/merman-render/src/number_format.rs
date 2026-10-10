const MAX_SAFE_INTEGER_F64: f64 = 9_007_199_254_740_991.0;

/// Removes non-semantic floating-point noise before a number enters SVG or CSS.
///
/// Theme sizes originate as `f32`, while layout and rendering use `f64`. Values close to an
/// integer must therefore converge before measurement, serialization, and receipts consume them.
pub(crate) fn canonicalize_number(mut value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    if value.abs() < 1e-9 {
        value = 0.0;
    }
    let nearest = value.round();
    if (value - nearest).abs() < 1e-6 {
        value = nearest;
    }
    if value == -0.0 { 0.0 } else { value }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CanonicalNumber(f64);

pub(crate) fn canonical_number(value: f64) -> CanonicalNumber {
    CanonicalNumber(canonicalize_number(value))
}

impl std::fmt::Display for CanonicalNumber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.0;
        if value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER_F64 {
            return write!(formatter, "{}", value as i64);
        }
        write!(formatter, "{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_number_removes_promoted_f32_integer_noise() {
        let below_sixteen = f64::from(f32::from_bits(0x417f_ffff));

        assert_eq!(canonicalize_number(below_sixteen), 16.0);
        assert_eq!(canonical_number(below_sixteen).to_string(), "16");
        assert_eq!(canonical_number(15.75).to_string(), "15.75");
    }
}
