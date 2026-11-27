use crate::svg_output::SvgOutput;

pub fn fmt_path_into(out: &mut impl SvgOutput, v: f64) {
    append_fixed_3dp_trimmed(out, path_coordinate_thousandths(v));
}

/// Write a path coordinate and return the value represented by those same bytes.
pub(super) fn fmt_path_coordinate_into(out: &mut String, v: f64) -> f64 {
    let thousandths = path_coordinate_thousandths(v);
    let start = out.len();
    append_fixed_3dp_trimmed(out, thousandths);
    let numerator = thousandths as f64;
    if thousandths.unsigned_abs() <= (1 << 53) {
        numerator / 1000.0
    } else {
        // Converting the numerator to f64 can round before the division at this magnitude.
        out[start..].parse().unwrap_or(numerator / 1000.0)
    }
}

pub fn path_coordinate_thousandths(v: f64) -> i64 {
    // D3's `d3-path` defaults to 3 fractional digits when stringifying path commands.
    // Upstream Mermaid fixtures match a `toFixed(3)`-like rounding behavior: round to nearest with
    // ties away from zero (not `Math.round`, which rounds negative halves toward +∞).
    if !v.is_finite() || v.abs() < 0.0005 {
        return 0;
    }

    let scaled = v * 1000.0;
    if scaled < 0.0 {
        (scaled - 0.5).ceil() as i64
    } else {
        (scaled + 0.5).floor() as i64
    }
}

fn append_fixed_3dp_trimmed(out: &mut impl SvgOutput, k: i64) {
    if k == 0 {
        out.push('0');
        return;
    }

    let neg = k.is_negative();
    let abs = k.unsigned_abs();
    let int_part = abs / 1000;
    let frac = abs % 1000;

    if neg {
        out.push('-');
    }

    let _ = write!(out, "{int_part}");

    if frac == 0 {
        return;
    }

    let mut frac_str = [b'0'; 3];
    frac_str[0] = b'0' + ((frac / 100) as u8);
    frac_str[1] = b'0' + (((frac / 10) % 10) as u8);
    frac_str[2] = b'0' + ((frac % 10) as u8);

    let mut end = 3usize;
    while end > 0 && frac_str[end - 1] == b'0' {
        end -= 1;
    }
    if end == 0 {
        return;
    }

    out.push('.');
    for &b in &frac_str[..end] {
        out.push(b as char);
    }
}
