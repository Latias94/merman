use crate::MermaidConfig;
use ryu_js::Buffer;
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy)]
struct Rgb01 {
    r: f64,
    g: f64,
    b: f64,
}

#[derive(Debug, Clone, Copy)]
struct Hsl {
    h_deg: f64,
    s_pct: f64,
    l_pct: f64,
}

fn round_1e10(v: f64) -> f64 {
    let v = (v * 1e10).round() / 1e10;
    if v == -0.0 { 0.0 } else { v }
}

fn fmt_js_1e10(v: f64) -> String {
    let v = round_1e10(v);
    let mut b = Buffer::new();
    b.format_finite(v).to_string()
}

fn round_hsl_1e10(mut hsl: Hsl) -> Hsl {
    hsl.h_deg = round_1e10(hsl.h_deg);
    hsl.h_deg %= 360.0;
    if hsl.h_deg < 0.0 {
        hsl.h_deg += 360.0;
    }
    hsl.s_pct = round_1e10(hsl.s_pct).clamp(0.0, 100.0);
    hsl.l_pct = round_1e10(hsl.l_pct).clamp(0.0, 100.0);
    hsl
}

fn parse_hex_rgb01(s: &str) -> Option<Rgb01> {
    let s = s.trim();
    let hex = s.strip_prefix('#')?;
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
    Some(Rgb01 {
        r: (r as f64) / 255.0,
        g: (g as f64) / 255.0,
        b: (b as f64) / 255.0,
    })
}

fn rgb01_to_hex(rgb: Rgb01) -> String {
    let r = (rgb.r.clamp(0.0, 1.0) * 255.0).round() as i64;
    let g = (rgb.g.clamp(0.0, 1.0) * 255.0).round() as i64;
    let b = (rgb.b.clamp(0.0, 1.0) * 255.0).round() as i64;
    format!(
        "#{:02x}{:02x}{:02x}",
        r.clamp(0, 255),
        g.clamp(0, 255),
        b.clamp(0, 255)
    )
}

fn rgb01_to_hsl(rgb: Rgb01) -> Hsl {
    let r = rgb.r;
    let g = rgb.g;
    let b = rgb.b;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if max == min {
        return round_hsl_1e10(Hsl {
            h_deg: 0.0,
            s_pct: 0.0,
            l_pct: l * 100.0,
        });
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let mut h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h /= 6.0;

    round_hsl_1e10(Hsl {
        h_deg: h * 360.0,
        s_pct: s * 100.0,
        l_pct: l * 100.0,
    })
}

fn adjust_hsl(mut hsl: Hsl, h_delta: f64, s_delta: f64, l_delta: f64) -> Hsl {
    let mut h = hsl.h_deg + h_delta;
    h %= 360.0;
    if h < 0.0 {
        h += 360.0;
    }
    hsl.h_deg = h;
    hsl.s_pct = (hsl.s_pct + s_delta).clamp(0.0, 100.0);
    hsl.l_pct = (hsl.l_pct + l_delta).clamp(0.0, 100.0);
    round_hsl_1e10(hsl)
}

fn fmt_hsl(hsl: Hsl) -> String {
    format!(
        "hsl({}, {}%, {}%)",
        fmt_js_1e10(hsl.h_deg),
        fmt_js_1e10(hsl.s_pct),
        fmt_js_1e10(hsl.l_pct)
    )
}

fn hsl_to_rgb01(hsl: Hsl) -> Rgb01 {
    let h = (hsl.h_deg / 360.0) % 1.0;
    let s = (hsl.s_pct / 100.0).clamp(0.0, 1.0);
    let l = (hsl.l_pct / 100.0).clamp(0.0, 1.0);

    if s == 0.0 {
        return Rgb01 { r: l, g: l, b: l };
    }

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

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    Rgb01 {
        r: hue_to_rgb(p, q, h + 1.0 / 3.0),
        g: hue_to_rgb(p, q, h),
        b: hue_to_rgb(p, q, h - 1.0 / 3.0),
    }
}

fn invert_rgb01_to_rgb_string(rgb: Rgb01) -> String {
    let r = round_1e10((1.0 - rgb.r) * 255.0);
    let g = round_1e10((1.0 - rgb.g) * 255.0);
    let b = round_1e10((1.0 - rgb.b) * 255.0);
    format!(
        "rgb({}, {}, {})",
        fmt_js_1e10(r),
        fmt_js_1e10(g),
        fmt_js_1e10(b)
    )
}

fn get_truthy_string(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn set_if_missing(map: &mut Map<String, Value>, key: &str, value: Value) {
    let is_missing = match map.get(key) {
        None => true,
        Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        _ => false,
    };
    if is_missing {
        map.insert(key.to_string(), value);
    }
}

pub(crate) fn apply_theme_defaults(config: &mut MermaidConfig) {
    let theme = config.get_str("theme").unwrap_or("default");
    if theme != "base" {
        return;
    }

    let mut tv = match config.as_value().get("themeVariables") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };

    let dark_mode = tv
        .get("darkMode")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let background = get_truthy_string(&tv, "background").unwrap_or_else(|| "#f4f4f4".to_string());
    let primary_color =
        get_truthy_string(&tv, "primaryColor").unwrap_or_else(|| "#fff4dd".to_string());

    set_if_missing(
        &mut tv,
        "primaryTextColor",
        Value::String(if dark_mode { "#eee" } else { "#333" }.to_string()),
    );

    let primary_text_color = get_truthy_string(&tv, "primaryTextColor")
        .unwrap_or_else(|| if dark_mode { "#eee" } else { "#333" }.to_string());

    let primary_hsl = parse_hex_rgb01(&primary_color)
        .map(rgb01_to_hsl)
        .unwrap_or(Hsl {
            h_deg: 0.0,
            s_pct: 0.0,
            l_pct: 100.0,
        });

    let secondary_hsl = if let Some(v) =
        get_truthy_string(&tv, "secondaryColor").and_then(|s| parse_hex_rgb01(&s).map(rgb01_to_hsl))
    {
        v
    } else {
        adjust_hsl(primary_hsl, -120.0, 0.0, 0.0)
    };
    set_if_missing(
        &mut tv,
        "secondaryColor",
        Value::String(fmt_hsl(secondary_hsl)),
    );

    let tertiary_hsl = if let Some(v) =
        get_truthy_string(&tv, "tertiaryColor").and_then(|s| parse_hex_rgb01(&s).map(rgb01_to_hsl))
    {
        v
    } else {
        adjust_hsl(primary_hsl, 180.0, 0.0, 5.0)
    };
    set_if_missing(
        &mut tv,
        "tertiaryColor",
        Value::String(fmt_hsl(tertiary_hsl)),
    );

    let primary_border_hsl = if get_truthy_string(&tv, "primaryBorderColor").is_some() {
        None
    } else {
        Some(adjust_hsl(
            primary_hsl,
            0.0,
            -40.0,
            if dark_mode { 10.0 } else { -10.0 },
        ))
    };
    if let Some(hsl) = primary_border_hsl {
        tv.insert(
            "primaryBorderColor".to_string(),
            Value::String(fmt_hsl(hsl)),
        );
    }

    let tertiary_border_hsl = if get_truthy_string(&tv, "tertiaryBorderColor").is_some() {
        None
    } else {
        Some(adjust_hsl(
            tertiary_hsl,
            0.0,
            -40.0,
            if dark_mode { 10.0 } else { -10.0 },
        ))
    };
    if let Some(hsl) = tertiary_border_hsl {
        tv.insert(
            "tertiaryBorderColor".to_string(),
            Value::String(fmt_hsl(hsl)),
        );
    }

    if get_truthy_string(&tv, "lineColor").is_none() {
        if let Some(bg_rgb) = parse_hex_rgb01(&background) {
            tv.insert(
                "lineColor".to_string(),
                Value::String(rgb01_to_hex(Rgb01 {
                    r: 1.0 - bg_rgb.r,
                    g: 1.0 - bg_rgb.g,
                    b: 1.0 - bg_rgb.b,
                })),
            );
        }
    }
    let line_color = get_truthy_string(&tv, "lineColor").unwrap_or_else(|| "#333333".to_string());
    set_if_missing(&mut tv, "arrowheadColor", Value::String(line_color));

    set_if_missing(
        &mut tv,
        "textColor",
        Value::String(primary_text_color.clone()),
    );

    let primary_border_color =
        get_truthy_string(&tv, "primaryBorderColor").unwrap_or_else(|| "#9370DB".to_string());
    let tertiary_border_color =
        get_truthy_string(&tv, "tertiaryBorderColor").unwrap_or_else(|| "#aaaa33".to_string());
    let tertiary_color = get_truthy_string(&tv, "tertiaryColor")
        .unwrap_or_else(|| "hsl(80, 100%, 96.2745098039%)".to_string());

    set_if_missing(&mut tv, "nodeBkg", Value::String(primary_color.clone()));
    set_if_missing(&mut tv, "mainBkg", Value::String(primary_color.clone()));
    set_if_missing(&mut tv, "nodeBorder", Value::String(primary_border_color));
    set_if_missing(&mut tv, "clusterBkg", Value::String(tertiary_color.clone()));
    set_if_missing(
        &mut tv,
        "clusterBorder",
        Value::String(tertiary_border_color),
    );
    set_if_missing(&mut tv, "nodeTextColor", Value::String(primary_text_color));

    if get_truthy_string(&tv, "tertiaryTextColor").is_none() {
        let rgb = hsl_to_rgb01(tertiary_hsl);
        tv.insert(
            "tertiaryTextColor".to_string(),
            Value::String(invert_rgb01_to_rgb_string(rgb)),
        );
    }
    let tertiary_text_color =
        get_truthy_string(&tv, "tertiaryTextColor").unwrap_or_else(|| "#333".to_string());
    set_if_missing(
        &mut tv,
        "titleColor",
        Value::String(tertiary_text_color.clone()),
    );

    if get_truthy_string(&tv, "edgeLabelBackground").is_none() {
        let mut v = secondary_hsl;
        if dark_mode {
            v = adjust_hsl(v, 0.0, 0.0, -30.0);
        }
        tv.insert("edgeLabelBackground".to_string(), Value::String(fmt_hsl(v)));
    }

    set_if_missing(&mut tv, "errorBkgColor", Value::String(tertiary_color));
    set_if_missing(
        &mut tv,
        "errorTextColor",
        Value::String(tertiary_text_color),
    );

    config.set_value("themeVariables", Value::Object(tv));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn base_theme_derivation_matches_upstream_fixture_values() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": {
                "primaryColor": "#411d4e",
                "titleColor": "white",
                "darkMode": true
            }
        }));
        apply_theme_defaults(&mut cfg);

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("textColor").and_then(|v| v.as_str()), Some("#eee"));
        assert_eq!(
            tv.get("lineColor").and_then(|v| v.as_str()),
            Some("#0b0b0b")
        );
        assert_eq!(
            tv.get("nodeBorder").and_then(|v| v.as_str()),
            Some("hsl(284.0816326531, 5.7943925234%, 30.9803921569%)")
        );
        assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#411d4e"));
        assert_eq!(
            tv.get("clusterBkg").and_then(|v| v.as_str()),
            Some("hsl(104.0816326531, 45.7943925234%, 25.9803921569%)")
        );
        assert_eq!(
            tv.get("clusterBorder").and_then(|v| v.as_str()),
            Some("hsl(104.0816326531, 5.7943925234%, 35.9803921569%)")
        );
        assert_eq!(
            tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
            Some("hsl(164.0816326531, 45.7943925234%, 0%)")
        );
        assert_eq!(
            tv.get("errorBkgColor").and_then(|v| v.as_str()),
            Some("hsl(104.0816326531, 45.7943925234%, 25.9803921569%)")
        );
        assert_eq!(
            tv.get("errorTextColor").and_then(|v| v.as_str()),
            Some("rgb(202.9906542056, 158.4112149531, 219.0887850467)")
        );
        assert_eq!(tv.get("titleColor").and_then(|v| v.as_str()), Some("white"));
    }
}
