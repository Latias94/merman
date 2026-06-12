pub(super) fn invert_hex_color(s: &str) -> Option<String> {
    let s = s.trim();
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(format!("#{:02x}{:02x}{:02x}", 255 - r, 255 - g, 255 - b))
}

pub(super) fn invert_timeline_label_color_to_hex(color: &str) -> Option<String> {
    let color = color.trim();
    if color.is_empty() {
        return None;
    }
    if color.eq_ignore_ascii_case("black") {
        return Some("#ffffff".to_string());
    }
    if color.eq_ignore_ascii_case("white") {
        return Some("#000000".to_string());
    }
    let hex = color.strip_prefix('#')?.trim();
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some(format!("#{:02x}{:02x}{:02x}", 255 - r, 255 - g, 255 - b))
}

pub(super) fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let t = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&t[0..2], 16).ok()?;
    let g = u8::from_str_radix(&t[2..4], 16).ok()?;
    let b = u8::from_str_radix(&t[4..6], 16).ok()?;
    Some((r, g, b))
}

pub(super) fn parse_venn_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.trim().strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some((r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

pub(super) fn parse_venn_css_rgb(s: &str) -> Option<(u8, u8, u8)> {
    parse_venn_hex_rgb(s).or_else(|| parse_rgb_css(s))
}

pub(super) fn venn_luminance(r: u8, g: u8, b: u8) -> f64 {
    fn linear(channel: u8) -> f64 {
        let v = channel as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

pub(super) fn adjust_hex_rgb(hex: &str, delta: i16) -> Option<String> {
    let (r, g, b) = parse_hex_rgb(hex)?;
    let adj = |c: u8| -> u8 {
        let v = c as i16 + delta;
        v.clamp(0, 255) as u8
    };
    Some(format!("#{:02x}{:02x}{:02x}", adj(r), adj(g), adj(b)))
}

fn round_1e10(v: f64) -> f64 {
    let v = (v * 1e10).round() / 1e10;
    if v == -0.0 { 0.0 } else { v }
}

fn format_hsl_css(h: f64, s: f64, l: f64, buf: &mut ryu_js::Buffer) -> String {
    let h = buf.format_finite(round_1e10(h)).to_string();
    let s = buf.format_finite(round_1e10(s)).to_string();
    let l = buf.format_finite(round_1e10(l)).to_string();
    format!("hsl({h}, {s}%, {l}%)")
}

pub(super) fn derive_timeline_c_scale_inv_fallback(
    c_scale: &str,
    buf: &mut ryu_js::Buffer,
) -> Option<String> {
    let (h, s, l) = parse_hsl_css(c_scale)?;
    let h = (h + 180.0) % 360.0;
    let l = (l + 10.0).clamp(0.0, 100.0);
    Some(format_hsl_css(h, s, l, buf))
}

pub(super) fn default_c_scale(i: usize) -> &'static str {
    match i {
        0 => "hsl(240, 100%, 76.2745098039%)",
        1 => "hsl(60, 100%, 73.5294117647%)",
        2 => "hsl(80, 100%, 76.2745098039%)",
        3 => "hsl(270, 100%, 76.2745098039%)",
        4 => "hsl(300, 100%, 76.2745098039%)",
        5 => "hsl(330, 100%, 76.2745098039%)",
        6 => "hsl(0, 100%, 76.2745098039%)",
        7 => "hsl(30, 100%, 76.2745098039%)",
        8 => "hsl(90, 100%, 76.2745098039%)",
        9 => "hsl(150, 100%, 76.2745098039%)",
        10 => "hsl(180, 100%, 76.2745098039%)",
        _ => "hsl(210, 100%, 76.2745098039%)",
    }
}

pub(super) fn default_c_scale_peer(i: usize) -> &'static str {
    match i {
        0 => "hsl(240, 100%, 61.2745098039%)",
        1 => "hsl(60, 100%, 48.5294117647%)",
        2 => "hsl(80, 100%, 56.2745098039%)",
        3 => "hsl(270, 100%, 61.2745098039%)",
        4 => "hsl(300, 100%, 61.2745098039%)",
        5 => "hsl(330, 100%, 61.2745098039%)",
        6 => "hsl(0, 100%, 61.2745098039%)",
        7 => "hsl(30, 100%, 61.2745098039%)",
        8 => "hsl(90, 100%, 61.2745098039%)",
        9 => "hsl(150, 100%, 61.2745098039%)",
        10 => "hsl(180, 100%, 61.2745098039%)",
        _ => "hsl(210, 100%, 61.2745098039%)",
    }
}

pub(super) fn default_c_scale_inv(i: usize) -> &'static str {
    match i {
        0 => "hsl(60, 100%, 86.2745098039%)",
        1 => "hsl(240, 100%, 83.5294117647%)",
        2 => "hsl(260, 100%, 86.2745098039%)",
        3 => "hsl(90, 100%, 86.2745098039%)",
        4 => "hsl(120, 100%, 86.2745098039%)",
        5 => "hsl(150, 100%, 86.2745098039%)",
        6 => "hsl(180, 100%, 86.2745098039%)",
        7 => "hsl(210, 100%, 86.2745098039%)",
        8 => "hsl(270, 100%, 86.2745098039%)",
        9 => "hsl(330, 100%, 86.2745098039%)",
        10 => "hsl(0, 100%, 86.2745098039%)",
        _ => "hsl(30, 100%, 86.2745098039%)",
    }
}

pub(super) fn default_c_scale_label(i: usize) -> &'static str {
    match i {
        0 | 3 => "#ffffff",
        _ => "black",
    }
}

pub(super) fn adjust_kanban_section_fill(
    c_scale: &str,
    dark_mode: bool,
    buf: &mut ryu_js::Buffer,
) -> Option<String> {
    let (h, s, l) = parse_hsl_css(c_scale)?;
    let delta = if dark_mode { -10.0 } else { 10.0 };
    Some(format_hsl_css(h, s, (l + delta).clamp(0.0, 100.0), buf))
}

pub(super) fn journey_default_fill_type(i: usize) -> &'static str {
    match i {
        0 => "#ECECFF",
        1 => "#ffffde",
        2 => "hsl(304, 100%, 96.2745098039%)",
        3 => "hsl(124, 100%, 93.5294117647%)",
        4 => "hsl(176, 100%, 96.2745098039%)",
        5 => "hsl(-4, 100%, 93.5294117647%)",
        6 => "hsl(8, 100%, 96.2745098039%)",
        _ => "hsl(188, 100%, 93.5294117647%)",
    }
}

fn fmt_rgb(r: u8, g: u8, b: u8) -> String {
    format!("rgb({r}, {g}, {b})")
}

fn parse_rgb_css(s: &str) -> Option<(u8, u8, u8)> {
    let inner = s.trim().strip_prefix("rgb(")?.strip_suffix(')')?;
    let mut parts = inner.split(',').map(|p| p.trim());
    let parse_channel = |part: &str| -> Option<u8> {
        let value = part.parse::<f64>().ok()?;
        if !value.is_finite() {
            return None;
        }
        Some(value.round().clamp(0.0, 255.0) as u8)
    };
    let r = parse_channel(parts.next()?)?;
    let g = parse_channel(parts.next()?)?;
    let b = parse_channel(parts.next()?)?;
    Some((r, g, b))
}

fn parse_hsl_css(s: &str) -> Option<(f64, f64, f64)> {
    let inner = s.trim().strip_prefix("hsl(")?.strip_suffix(')')?;
    let mut parts = inner.split(',').map(|p| p.trim());
    let h = parts.next()?.parse::<f64>().ok()?;
    let s = parts
        .next()?
        .strip_suffix('%')
        .unwrap_or_default()
        .parse::<f64>()
        .ok()?;
    let l = parts
        .next()?
        .strip_suffix('%')
        .unwrap_or_default()
        .parse::<f64>()
        .ok()?;
    Some((h, s, l))
}

fn hsl_to_rgb_u8(h_deg: f64, s_pct: f64, l_pct: f64) -> Option<(u8, u8, u8)> {
    if !(h_deg.is_finite() && s_pct.is_finite() && l_pct.is_finite()) {
        return None;
    }

    let h = (h_deg / 360.0).rem_euclid(1.0);
    let s = (s_pct / 100.0).clamp(0.0, 1.0);
    let l = (l_pct / 100.0).clamp(0.0, 1.0);

    if s == 0.0 {
        let v = (l * 255.0).round().clamp(0.0, 255.0) as u8;
        return Some((v, v, v));
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    }

    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);

    let to_u8 = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    Some((to_u8(r), to_u8(g), to_u8(b)))
}

fn css_color_to_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let t = s.trim();
    if let Some(rgb) = parse_rgb_css(t) {
        return Some(rgb);
    }
    if let Some(rgb) = parse_hex_rgb(t) {
        return Some(rgb);
    }
    if let Some((h, s, l)) = parse_hsl_css(t) {
        return hsl_to_rgb_u8(h, s, l);
    }
    None
}

pub(super) fn css_color_to_rgb_string(s: &str) -> Option<String> {
    let (r, g, b) = css_color_to_rgb(s)?;
    Some(fmt_rgb(r, g, b))
}

pub(super) fn parse_treemap_css_rgb(color: &str) -> Option<(u8, u8, u8)> {
    let color = color.trim();
    if color.eq_ignore_ascii_case("black") {
        return Some((0, 0, 0));
    }
    if color.eq_ignore_ascii_case("white") {
        return Some((255, 255, 255));
    }
    if let Some(hex) = color.strip_prefix('#') {
        let hex = hex.trim();
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            return Some((r, g, b));
        }
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some((r, g, b));
        }
    }
    let lower = color.to_ascii_lowercase();
    if let Some(args) = lower
        .strip_prefix("rgb(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let parts = args
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() >= 3 {
            let r = parts[0].parse::<u16>().ok()?;
            let g = parts[1].parse::<u16>().ok()?;
            let b = parts[2].parse::<u16>().ok()?;
            if r <= 255 && g <= 255 && b <= 255 {
                return Some((r as u8, g as u8, b as u8));
            }
        }
    }
    None
}

pub(super) fn invert_treemap_label_color_to_hex(color: &str) -> Option<String> {
    let (r, g, b) = parse_treemap_css_rgb(color)?;
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        255u8.saturating_sub(r),
        255u8.saturating_sub(g),
        255u8.saturating_sub(b)
    ))
}

pub(super) fn css_color_is_transparent(color: &str) -> bool {
    color.trim().eq_ignore_ascii_case("transparent")
}

pub(super) fn css_color_is_white_like(color: &str) -> bool {
    parse_treemap_css_rgb(color).is_some_and(|(r, g, b)| r >= 250 && g >= 250 && b >= 250)
}

pub(super) fn style_has_non_empty_decl(style: &str, property: &str) -> bool {
    style.split(';').any(|decl| {
        let Some((key, value)) = decl.split_once(':') else {
            return false;
        };
        key.trim().eq_ignore_ascii_case(property) && !value.trim().is_empty()
    })
}

fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    fn to_linear(channel: u8) -> f64 {
        let v = channel as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
}

fn rgb_to_hsl_pct(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f64::EPSILON {
        return (0.0, 0.0, l * 100.0);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < f64::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f64::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;

    (h * 360.0, s * 100.0, l * 100.0)
}

pub(super) fn derive_quadrant_point_fill(quadrant1_fill: &str, fallback: &str) -> String {
    let Some((r, g, b)) = css_color_to_rgb(quadrant1_fill) else {
        return fallback.to_string();
    };
    let (h, s, l) = rgb_to_hsl_pct(r, g, b);
    let delta = if relative_luminance(r, g, b) < 0.5 {
        10.0
    } else {
        -10.0
    };
    let adjusted_l = (l + delta).clamp(0.0, 100.0);
    let Some((r, g, b)) = hsl_to_rgb_u8(h, s, adjusted_l) else {
        return fallback.to_string();
    };
    fmt_rgb(r, g, b)
}

pub(super) fn is_invalid_css_token(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.is_empty()
        || lower.contains("nan")
        || lower.contains("undefined")
        || lower.contains("infinity")
}
