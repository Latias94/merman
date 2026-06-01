use crate::MermaidConfig;
use ryu_js::Buffer;
use serde_json::{Map, Value};

pub(crate) const SUPPORTED_THEME_NAMES: &[&str] = &["default", "base", "dark", "forest", "neutral"];

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
    // Match Mermaid's base theme output: wrap using remainder without forcing positive hue.
    // (JS `%` keeps the sign, so negative hues remain negative.)
    hsl.h_deg = round_1e10(hsl.h_deg) % 360.0;
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

fn invert_rgb01_to_hex(rgb: Rgb01) -> String {
    rgb01_to_hex(Rgb01 {
        r: 1.0 - rgb.r,
        g: 1.0 - rgb.g,
        b: 1.0 - rgb.b,
    })
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
    hsl.h_deg = (hsl.h_deg + h_delta) % 360.0;
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

fn theme_variables_map(config: &MermaidConfig) -> Map<String, Value> {
    match config.as_value().get("themeVariables") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    }
}

fn mermaid_default_font_family() -> Value {
    Value::String("\"trebuchet ms\", verdana, arial, sans-serif".to_string())
}

fn mk_border_hsl(hsl: Hsl, dark_mode: bool) -> String {
    fmt_hsl(adjust_hsl(
        hsl,
        0.0,
        -40.0,
        if dark_mode { 10.0 } else { -10.0 },
    ))
}

fn ensure_gradient_theme_defaults(tv: &mut Map<String, Value>) {
    let primary_border_color =
        get_truthy_string(tv, "primaryBorderColor").unwrap_or_else(|| "#9370DB".to_string());
    let secondary_border_color = get_truthy_string(tv, "secondaryBorderColor")
        .unwrap_or_else(|| primary_border_color.clone());

    set_if_missing(tv, "useGradient", Value::Bool(true));
    set_if_missing(tv, "gradientStart", Value::String(primary_border_color));
    set_if_missing(tv, "gradientStop", Value::String(secondary_border_color));
}

fn ensure_xychart_theme_defaults(tv: &mut Map<String, Value>, default_palette: &str) {
    let background = get_truthy_string(tv, "background").unwrap_or_else(|| "white".to_string());
    let primary_text = get_truthy_string(tv, "primaryTextColor")
        .or_else(|| get_truthy_string(tv, "textColor"))
        .unwrap_or_else(|| "#333".to_string());

    let mut xy = match tv.get("xyChart") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };

    set_if_missing(
        &mut xy,
        "backgroundColor",
        Value::String(background.clone()),
    );
    for key in [
        "titleColor",
        "dataLabelColor",
        "xAxisTitleColor",
        "xAxisLabelColor",
        "xAxisTickColor",
        "xAxisLineColor",
        "yAxisTitleColor",
        "yAxisLabelColor",
        "yAxisTickColor",
        "yAxisLineColor",
    ] {
        set_if_missing(&mut xy, key, Value::String(primary_text.clone()));
    }
    set_if_missing(
        &mut xy,
        "plotColorPalette",
        Value::String(default_palette.to_string()),
    );

    tv.insert("xyChart".to_string(), Value::Object(xy));
}

pub(crate) fn apply_theme_defaults(config: &mut MermaidConfig) {
    let theme = config.get_str("theme").unwrap_or("default");
    match theme {
        "default" => apply_default_theme_defaults(config),
        "base" => apply_base_theme_defaults(config),
        "dark" => apply_dark_theme_defaults(config),
        "forest" => apply_forest_theme_defaults(config),
        "neutral" => apply_neutral_theme_defaults(config),
        _ => apply_default_theme_defaults(config),
    }
}

fn apply_default_theme_defaults(config: &mut MermaidConfig) {
    let mut tv = theme_variables_map(config);

    // Mermaid 11.12.3: `theme-default` constructor defaults and `updateColors()`.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-default.js`.
    let default_primary = "#ECECFF";
    let default_secondary = "#ffffde";
    let default_primary_hsl = parse_hex_rgb01(default_primary)
        .map(rgb01_to_hsl)
        .unwrap_or(Hsl {
            h_deg: 240.0,
            s_pct: 100.0,
            l_pct: 96.2745098039,
        });
    let default_secondary_hsl = parse_hex_rgb01(default_secondary)
        .map(rgb01_to_hsl)
        .unwrap_or(Hsl {
            h_deg: 60.0,
            s_pct: 100.0,
            l_pct: 93.5294117647,
        });
    let default_tertiary_hsl = adjust_hsl(default_primary_hsl, -160.0, 0.0, 0.0);

    set_if_missing(&mut tv, "background", Value::String("white".to_string()));
    set_if_missing(
        &mut tv,
        "primaryColor",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "secondaryColor",
        Value::String(default_secondary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "tertiaryColor",
        Value::String(fmt_hsl(default_tertiary_hsl)),
    );

    set_if_missing(
        &mut tv,
        "primaryBorderColor",
        Value::String(mk_border_hsl(default_primary_hsl, false)),
    );
    set_if_missing(
        &mut tv,
        "secondaryBorderColor",
        Value::String(mk_border_hsl(default_secondary_hsl, false)),
    );
    set_if_missing(
        &mut tv,
        "tertiaryBorderColor",
        Value::String(mk_border_hsl(default_tertiary_hsl, false)),
    );

    set_if_missing(
        &mut tv,
        "primaryTextColor",
        Value::String("#131300".to_string()),
    );
    set_if_missing(
        &mut tv,
        "secondaryTextColor",
        Value::String("#000021".to_string()),
    );
    set_if_missing(
        &mut tv,
        "tertiaryTextColor",
        Value::String(invert_rgb01_to_rgb_string(hsl_to_rgb01(
            default_tertiary_hsl,
        ))),
    );

    set_if_missing(
        &mut tv,
        "mainBkg",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "secondBkg",
        Value::String(default_secondary.to_string()),
    );
    set_if_missing(&mut tv, "lineColor", Value::String("#333333".to_string()));
    set_if_missing(&mut tv, "border1", Value::String("#9370DB".to_string()));
    set_if_missing(&mut tv, "border2", Value::String("#aaaa33".to_string()));
    set_if_missing(
        &mut tv,
        "arrowheadColor",
        Value::String("#333333".to_string()),
    );
    set_if_missing(&mut tv, "fontFamily", mermaid_default_font_family());
    set_if_missing(&mut tv, "fontSize", Value::String("16px".to_string()));
    set_if_missing(
        &mut tv,
        "labelBackground",
        Value::String("rgba(232,232,232, 0.8)".to_string()),
    );
    set_if_missing(&mut tv, "textColor", Value::String("#333".to_string()));
    set_if_missing(&mut tv, "THEME_COLOR_LIMIT", Value::Number(12.into()));
    set_if_missing(&mut tv, "radius", Value::Number(5.into()));
    set_if_missing(&mut tv, "strokeWidth", Value::Number(1.into()));

    let main_bkg = get_truthy_string(&tv, "mainBkg").unwrap_or_else(|| default_primary.to_string());
    let second_bkg =
        get_truthy_string(&tv, "secondBkg").unwrap_or_else(|| default_secondary.to_string());
    let line_color = get_truthy_string(&tv, "lineColor").unwrap_or_else(|| "#333333".to_string());
    let text_color = get_truthy_string(&tv, "textColor").unwrap_or_else(|| "#333".to_string());
    let border1 = get_truthy_string(&tv, "border1").unwrap_or_else(|| "#9370DB".to_string());
    let border2 = get_truthy_string(&tv, "border2").unwrap_or_else(|| "#aaaa33".to_string());
    let label_background = get_truthy_string(&tv, "labelBackground")
        .unwrap_or_else(|| "rgba(232,232,232, 0.8)".to_string());
    let primary_text_color =
        get_truthy_string(&tv, "primaryTextColor").unwrap_or_else(|| "#131300".to_string());

    // Flowchart and block/class surfaces.
    set_if_missing(&mut tv, "nodeBkg", Value::String(main_bkg.clone()));
    set_if_missing(&mut tv, "nodeBorder", Value::String(border1.clone()));
    set_if_missing(&mut tv, "clusterBkg", Value::String(second_bkg.clone()));
    set_if_missing(&mut tv, "clusterBorder", Value::String(border2.clone()));
    set_if_missing(
        &mut tv,
        "defaultLinkColor",
        Value::String(line_color.clone()),
    );
    set_if_missing(&mut tv, "titleColor", Value::String(text_color.clone()));
    set_if_missing(
        &mut tv,
        "edgeLabelBackground",
        Value::String(label_background.clone()),
    );
    set_if_missing(
        &mut tv,
        "nodeTextColor",
        Value::String(primary_text_color.clone()),
    );

    // Sequence diagram surfaces.
    set_if_missing(&mut tv, "actorBorder", Value::String(border1.clone()));
    set_if_missing(&mut tv, "actorBkg", Value::String(main_bkg.clone()));
    set_if_missing(
        &mut tv,
        "actorTextColor",
        Value::String("black".to_string()),
    );
    let actor_text_color =
        get_truthy_string(&tv, "actorTextColor").unwrap_or_else(|| "black".to_string());
    set_if_missing(&mut tv, "actorLineColor", Value::String(border1.clone()));
    set_if_missing(&mut tv, "labelBoxBkgColor", Value::String(main_bkg.clone()));
    set_if_missing(&mut tv, "signalColor", Value::String(text_color.clone()));
    set_if_missing(
        &mut tv,
        "signalTextColor",
        Value::String(text_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "labelBoxBorderColor",
        Value::String(border1.clone()),
    );
    set_if_missing(
        &mut tv,
        "labelTextColor",
        Value::String(actor_text_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "loopTextColor",
        Value::String(actor_text_color.clone()),
    );
    set_if_missing(&mut tv, "noteBorderColor", Value::String(border2.clone()));
    set_if_missing(
        &mut tv,
        "noteBkgColor",
        Value::String("#fff5ad".to_string()),
    );
    set_if_missing(
        &mut tv,
        "noteTextColor",
        Value::String(actor_text_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "activationBorderColor",
        Value::String("#666".to_string()),
    );
    set_if_missing(
        &mut tv,
        "activationBkgColor",
        Value::String("#f4f4f4".to_string()),
    );
    set_if_missing(
        &mut tv,
        "sequenceNumberColor",
        Value::String("white".to_string()),
    );
    let rect_bkg_color = get_truthy_string(&tv, "tertiaryColor")
        .map(Value::String)
        .unwrap_or_else(|| Value::String(fmt_hsl(default_tertiary_hsl)));
    set_if_missing(&mut tv, "rectBkgColor", rect_bkg_color);

    // Gantt chart surfaces.
    for (key, value) in [
        ("sectionBkgColor", "rgba(102, 102, 255, 0.49)"),
        ("altSectionBkgColor", "white"),
        ("sectionBkgColor2", "#fff400"),
        ("excludeBkgColor", "#eeeeee"),
        ("taskBorderColor", "#534fbc"),
        ("taskBkgColor", "#8a90dd"),
        ("taskTextLightColor", "white"),
        ("taskTextColor", "white"),
        ("taskTextDarkColor", "black"),
        ("taskTextOutsideColor", "black"),
        ("taskTextClickableColor", "#003163"),
        ("activeTaskBorderColor", "#534fbc"),
        ("activeTaskBkgColor", "#bfc7ff"),
        ("gridColor", "lightgrey"),
        ("doneTaskBkgColor", "lightgrey"),
        ("doneTaskBorderColor", "grey"),
        ("critBorderColor", "#ff8888"),
        ("critBkgColor", "red"),
        ("todayLineColor", "red"),
        ("vertLineColor", "navy"),
        ("noteFontWeight", "normal"),
        ("fontWeight", "normal"),
    ] {
        set_if_missing(&mut tv, key, Value::String(value.to_string()));
    }

    // C4 and architecture defaults.
    let primary_border_color = get_truthy_string(&tv, "primaryBorderColor")
        .unwrap_or_else(|| mk_border_hsl(default_primary_hsl, false));
    let secondary_border_color = get_truthy_string(&tv, "secondaryBorderColor")
        .unwrap_or_else(|| mk_border_hsl(default_secondary_hsl, false));
    set_if_missing(
        &mut tv,
        "personBorder",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(&mut tv, "personBkg", Value::String(main_bkg.clone()));
    set_if_missing(&mut tv, "archEdgeColor", Value::String(line_color.clone()));
    set_if_missing(
        &mut tv,
        "archEdgeArrowColor",
        Value::String(line_color.clone()),
    );
    set_if_missing(&mut tv, "archEdgeWidth", Value::String("3".to_string()));
    set_if_missing(
        &mut tv,
        "archGroupBorderColor",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "archGroupBorderWidth",
        Value::String("2px".to_string()),
    );

    // ER, state, class, and requirement surfaces.
    set_if_missing(
        &mut tv,
        "rowOdd",
        Value::String(fmt_hsl(adjust_hsl(default_primary_hsl, 0.0, 0.0, 75.0))),
    );
    set_if_missing(
        &mut tv,
        "rowEven",
        Value::String(fmt_hsl(adjust_hsl(default_primary_hsl, 0.0, 0.0, 1.0))),
    );
    set_if_missing(
        &mut tv,
        "attributeBackgroundColorOdd",
        Value::String("#ffffff".to_string()),
    );
    set_if_missing(
        &mut tv,
        "attributeBackgroundColorEven",
        Value::String("#f2f2f2".to_string()),
    );
    set_if_missing(&mut tv, "labelColor", Value::String("black".to_string()));
    set_if_missing(
        &mut tv,
        "errorBkgColor",
        Value::String("#552222".to_string()),
    );
    set_if_missing(
        &mut tv,
        "errorTextColor",
        Value::String("#552222".to_string()),
    );
    set_if_missing(
        &mut tv,
        "transitionColor",
        Value::String(line_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "transitionLabelColor",
        Value::String(text_color.clone()),
    );
    let state_label_color = get_truthy_string(&tv, "stateBkg")
        .map(Value::String)
        .unwrap_or_else(|| Value::String(primary_text_color.clone()));
    set_if_missing(&mut tv, "stateLabelColor", state_label_color);
    set_if_missing(&mut tv, "stateBkg", Value::String(main_bkg.clone()));
    let state_bkg = get_truthy_string(&tv, "stateBkg").unwrap_or_else(|| main_bkg.clone());
    set_if_missing(&mut tv, "labelBackgroundColor", Value::String(state_bkg));
    let composite_background = get_truthy_string(&tv, "background")
        .map(Value::String)
        .unwrap_or_else(|| Value::String("white".to_string()));
    set_if_missing(&mut tv, "compositeBackground", composite_background);
    set_if_missing(
        &mut tv,
        "altBackground",
        Value::String("#f0f0f0".to_string()),
    );
    set_if_missing(
        &mut tv,
        "compositeTitleBackground",
        Value::String(main_bkg.clone()),
    );
    let node_border = get_truthy_string(&tv, "nodeBorder").unwrap_or_else(|| border1.clone());
    set_if_missing(
        &mut tv,
        "compositeBorder",
        Value::String(node_border.clone()),
    );
    set_if_missing(&mut tv, "innerEndBackground", Value::String(node_border));
    set_if_missing(
        &mut tv,
        "specialStateColor",
        Value::String(line_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "classText",
        Value::String(primary_text_color.clone()),
    );

    // Color scale.
    let primary_hsl = get_truthy_string(&tv, "primaryColor")
        .and_then(|s| parse_hex_rgb01(&s).map(rgb01_to_hsl))
        .unwrap_or(default_primary_hsl);
    let secondary_hsl = get_truthy_string(&tv, "secondaryColor")
        .and_then(|s| parse_hex_rgb01(&s).map(rgb01_to_hsl))
        .unwrap_or(default_secondary_hsl);
    let tertiary_hsl = get_truthy_string(&tv, "tertiaryColor")
        .and_then(|s| parse_hex_rgb01(&s).map(rgb01_to_hsl))
        .unwrap_or(default_tertiary_hsl);
    let c_scales: [Hsl; 12] = [
        primary_hsl,
        secondary_hsl,
        tertiary_hsl,
        adjust_hsl(primary_hsl, 30.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 60.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 90.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 120.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 150.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 210.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 270.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 300.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 330.0, 0.0, 0.0),
    ]
    .map(|base| adjust_hsl(base, 0.0, 0.0, -10.0));

    for (i, v) in c_scales.iter().enumerate() {
        set_if_missing(&mut tv, &format!("cScale{i}"), Value::String(fmt_hsl(*v)));
    }
    set_if_missing(
        &mut tv,
        "cScalePeer1",
        Value::String(fmt_hsl(adjust_hsl(secondary_hsl, 0.0, 0.0, -45.0))),
    );
    set_if_missing(
        &mut tv,
        "cScalePeer2",
        Value::String(fmt_hsl(adjust_hsl(tertiary_hsl, 0.0, 0.0, -40.0))),
    );
    for (i, c_hsl) in c_scales.iter().enumerate() {
        set_if_missing(
            &mut tv,
            &format!("cScalePeer{i}"),
            Value::String(fmt_hsl(adjust_hsl(*c_hsl, 0.0, 0.0, -25.0))),
        );
        set_if_missing(
            &mut tv,
            &format!("cScaleInv{i}"),
            Value::String(fmt_hsl(adjust_hsl(*c_hsl, 180.0, 0.0, 0.0))),
        );
        if i == 0 || i == 3 {
            set_if_missing(
                &mut tv,
                &format!("cScaleLabel{i}"),
                Value::String("#ffffff".to_string()),
            );
        }
        set_if_missing(
            &mut tv,
            &format!("cScaleLabel{i}"),
            Value::String(actor_text_color.clone()),
        );
    }
    set_if_missing(
        &mut tv,
        "scaleLabelColor",
        Value::String(actor_text_color.clone()),
    );

    // Journey and pie color defaults.
    for (key, value) in [
        ("fillType0", fmt_hsl(primary_hsl)),
        ("fillType1", fmt_hsl(secondary_hsl)),
        (
            "fillType2",
            fmt_hsl(adjust_hsl(primary_hsl, 64.0, 0.0, 0.0)),
        ),
        (
            "fillType3",
            fmt_hsl(adjust_hsl(secondary_hsl, 64.0, 0.0, 0.0)),
        ),
        (
            "fillType4",
            fmt_hsl(adjust_hsl(primary_hsl, -64.0, 0.0, 0.0)),
        ),
        (
            "fillType5",
            fmt_hsl(adjust_hsl(secondary_hsl, -64.0, 0.0, 0.0)),
        ),
        (
            "fillType6",
            fmt_hsl(adjust_hsl(primary_hsl, 128.0, 0.0, 0.0)),
        ),
        (
            "fillType7",
            fmt_hsl(adjust_hsl(secondary_hsl, 128.0, 0.0, 0.0)),
        ),
        ("pie1", fmt_hsl(primary_hsl)),
        ("pie2", fmt_hsl(secondary_hsl)),
        ("pie3", fmt_hsl(adjust_hsl(tertiary_hsl, 0.0, 0.0, -40.0))),
        ("pie4", fmt_hsl(adjust_hsl(primary_hsl, 0.0, 0.0, -10.0))),
        ("pie5", fmt_hsl(adjust_hsl(secondary_hsl, 0.0, 0.0, -30.0))),
        ("pie6", fmt_hsl(adjust_hsl(tertiary_hsl, 0.0, 0.0, -20.0))),
        ("pie7", fmt_hsl(adjust_hsl(primary_hsl, 60.0, 0.0, -20.0))),
        ("pie8", fmt_hsl(adjust_hsl(primary_hsl, -60.0, 0.0, -40.0))),
        ("pie9", fmt_hsl(adjust_hsl(primary_hsl, 120.0, 0.0, -40.0))),
        ("pie10", fmt_hsl(adjust_hsl(primary_hsl, 60.0, 0.0, -40.0))),
        ("pie11", fmt_hsl(adjust_hsl(primary_hsl, -90.0, 0.0, -40.0))),
        ("pie12", fmt_hsl(adjust_hsl(primary_hsl, 120.0, 0.0, -30.0))),
    ] {
        set_if_missing(&mut tv, key, Value::String(value));
    }
    for (key, value) in [
        ("pieTitleTextSize", "25px"),
        ("pieTitleTextColor", "black"),
        ("pieSectionTextSize", "17px"),
        ("pieSectionTextColor", text_color.as_str()),
        ("pieLegendTextSize", "17px"),
        ("pieLegendTextColor", "black"),
        ("pieStrokeColor", "black"),
        ("pieStrokeWidth", "2px"),
        ("pieOuterStrokeWidth", "2px"),
        ("pieOuterStrokeColor", "black"),
        ("pieOpacity", "0.7"),
    ] {
        set_if_missing(&mut tv, key, Value::String(value.to_string()));
    }

    // Requirement and git surfaces consumed by current renderers.
    set_if_missing(
        &mut tv,
        "requirementBackground",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "requirementBorderColor",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "requirementBorderSize",
        Value::String("1".to_string()),
    );
    set_if_missing(
        &mut tv,
        "requirementTextColor",
        Value::String(primary_text_color.clone()),
    );
    set_if_missing(&mut tv, "relationColor", Value::String(line_color.clone()));
    set_if_missing(
        &mut tv,
        "relationLabelBackground",
        Value::String(label_background),
    );
    set_if_missing(
        &mut tv,
        "relationLabelColor",
        Value::String(actor_text_color.clone()),
    );

    set_if_missing(&mut tv, "tagLabelColor", Value::String(primary_text_color));
    set_if_missing(
        &mut tv,
        "tagLabelBackground",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "tagLabelBorder",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "tagLabelFontSize",
        Value::String("10px".to_string()),
    );
    set_if_missing(
        &mut tv,
        "commitLabelColor",
        Value::String("#000021".to_string()),
    );
    set_if_missing(
        &mut tv,
        "commitLabelBackground",
        Value::String(default_secondary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "commitLabelFontSize",
        Value::String("10px".to_string()),
    );

    set_if_missing(&mut tv, "useGradient", Value::Bool(false));
    set_if_missing(
        &mut tv,
        "gradientStart",
        Value::String(primary_border_color),
    );
    set_if_missing(
        &mut tv,
        "gradientStop",
        Value::String(secondary_border_color),
    );
    set_if_missing(
        &mut tv,
        "dropShadow",
        Value::String("drop-shadow(1px 2px 2px rgba(185, 185, 185, 1))".to_string()),
    );

    ensure_xychart_theme_defaults(
        &mut tv,
        "#ECECFF,#8493A6,#FFC3A0,#DCDDE1,#B8E994,#D1A36F,#C3CDE6,#FFB6C1,#496078,#F8F3E3",
    );

    config.set_value("themeVariables", Value::Object(tv));
}

fn apply_dark_theme_defaults(config: &mut MermaidConfig) {
    let mut tv = theme_variables_map(config);

    // Mermaid 11.12.2: `theme-dark` color scale seeds.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-dark.js`.
    //
    // Note: `theme-dark` keeps `cScale*` as the provided hex strings, while derived
    // `cScalePeer*` values are produced via `khroma.lighten(...)` (serialized as `hsl(...)`).
    let c_scales_hex: [&str; 12] = [
        "#1f2020", // primaryColor
        "#0b0000", "#4d1037", "#3f5258", "#4f2f1b", "#6e0a0a", "#3b0048", "#995a01", "#154706",
        "#161722", "#00296f", "#01629c",
    ];

    // Minimal `theme-dark` seeds needed for diagram render parity when users set `theme: dark`.
    //
    // Mermaid's JS theme sets many more variables (and calculates derived values in `updateColors()`).
    // We seed the commonly-consumed surfaces + xychart palette here; other missing values are left
    // for future parity work as fixtures demand.
    set_if_missing(&mut tv, "background", Value::String("#333".to_string()));
    set_if_missing(
        &mut tv,
        "primaryColor",
        Value::String("#1f2020".to_string()),
    );
    if get_truthy_string(&tv, "primaryTextColor").is_none() {
        if let Some(primary_color) = get_truthy_string(&tv, "primaryColor") {
            if let Some(rgb) = parse_hex_rgb01(&primary_color) {
                tv.insert(
                    "primaryTextColor".to_string(),
                    Value::String(invert_rgb01_to_hex(rgb)),
                );
            }
        }
    }
    set_if_missing(&mut tv, "textColor", Value::String("#ccc".to_string()));
    set_if_missing(&mut tv, "fontFamily", mermaid_default_font_family());
    set_if_missing(&mut tv, "fontSize", Value::String("16px".to_string()));
    set_if_missing(&mut tv, "border1", Value::String("#ccc".to_string()));
    set_if_missing(
        &mut tv,
        "border2",
        Value::String("rgba(255, 255, 255, 0.25)".to_string()),
    );
    set_if_missing(
        &mut tv,
        "labelBackground",
        Value::String("#181818".to_string()),
    );
    set_if_missing(&mut tv, "titleColor", Value::String("#F9FFFE".to_string()));
    set_if_missing(
        &mut tv,
        "errorBkgColor",
        Value::String("#a44141".to_string()),
    );
    set_if_missing(&mut tv, "errorTextColor", Value::String("#ddd".to_string()));

    let primary_color =
        get_truthy_string(&tv, "primaryColor").unwrap_or_else(|| "#1f2020".to_string());
    let primary_hsl = parse_hex_rgb01(&primary_color)
        .map(rgb01_to_hsl)
        .unwrap_or(Hsl {
            h_deg: 180.0,
            s_pct: 1.5873015873,
            l_pct: 12.3529411765,
        });
    let secondary_hsl = adjust_hsl(primary_hsl, 0.0, 0.0, 16.0);
    set_if_missing(
        &mut tv,
        "secondaryColor",
        Value::String(fmt_hsl(secondary_hsl)),
    );
    set_if_missing(
        &mut tv,
        "primaryBorderColor",
        Value::String("#cccccc".to_string()),
    );
    set_if_missing(
        &mut tv,
        "secondaryBorderColor",
        Value::String(mk_border_hsl(secondary_hsl, false)),
    );
    ensure_gradient_theme_defaults(&mut tv);

    set_if_missing(
        &mut tv,
        "labelTextColor",
        Value::String("lightgrey".to_string()),
    );
    // Mermaid's `config.ts` calls `theme-dark.getThemeVariables(conf.themeVariables)` without
    // injecting `darkMode=true`, so `theme-dark.js` falls back to `labelTextColor` here.
    let label_text_color =
        get_truthy_string(&tv, "labelTextColor").unwrap_or_else(|| "lightgrey".to_string());
    set_if_missing(
        &mut tv,
        "scaleLabelColor",
        Value::String(label_text_color.clone()),
    );
    let scale_label_color =
        get_truthy_string(&tv, "scaleLabelColor").unwrap_or_else(|| label_text_color.clone());

    for (i, c_hex) in c_scales_hex.iter().enumerate() {
        set_if_missing(
            &mut tv,
            &format!("cScale{i}"),
            Value::String((*c_hex).to_string()),
        );

        let Some(rgb) = parse_hex_rgb01(c_hex) else {
            continue;
        };
        let hsl = rgb01_to_hsl(rgb);

        // `theme-dark` peers: `lighten(cScale, 10)`.
        set_if_missing(
            &mut tv,
            &format!("cScalePeer{i}"),
            Value::String(fmt_hsl(adjust_hsl(hsl, 0.0, 0.0, 10.0))),
        );

        // `theme-dark` inverted scale: `invert(cScale)`.
        set_if_missing(
            &mut tv,
            &format!("cScaleInv{i}"),
            Value::String(invert_rgb01_to_hex(rgb)),
        );

        // `theme-dark` label scale: `scaleLabelColor`.
        set_if_missing(
            &mut tv,
            &format!("cScaleLabel{i}"),
            Value::String(scale_label_color.clone()),
        );
    }

    // `theme-dark` xychart palette + colors.
    // Source: `theme-dark.js`.
    ensure_xychart_theme_defaults(
        &mut tv,
        "#3498db,#2ecc71,#e74c3c,#f1c40f,#bdc3c7,#ffffff,#34495e,#9b59b6,#1abc9c,#e67e22",
    );

    config.set_value("themeVariables", Value::Object(tv));
}

fn apply_forest_theme_defaults(config: &mut MermaidConfig) {
    let mut tv = theme_variables_map(config);

    // Mermaid 11.12.2: `theme-forest` base colors.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-forest.js`.
    //
    // NOTE: `theme-forest` is not a thin palette override. It sets several diagram-facing
    // variables (flowchart/state/sequence/...) in its `constructor()` + `updateColors()`.
    // We explicitly seed those values here so headless SVG rendering can match upstream.
    set_if_missing(
        &mut tv,
        "primaryColor",
        Value::String("#cde498".to_string()),
    );
    set_if_missing(
        &mut tv,
        "secondaryColor",
        Value::String("#cdffb2".to_string()),
    );
    set_if_missing(&mut tv, "background", Value::String("white".to_string()));
    set_if_missing(&mut tv, "border1", Value::String("#13540c".to_string()));
    set_if_missing(&mut tv, "border2", Value::String("#6eaa49".to_string()));
    set_if_missing(
        &mut tv,
        "arrowheadColor",
        Value::String("green".to_string()),
    );
    set_if_missing(&mut tv, "fontFamily", mermaid_default_font_family());
    set_if_missing(&mut tv, "fontSize", Value::String("16px".to_string()));
    set_if_missing(&mut tv, "titleColor", Value::String("#333".to_string()));
    set_if_missing(
        &mut tv,
        "edgeLabelBackground",
        Value::String("#e8e8e8".to_string()),
    );
    set_if_missing(
        &mut tv,
        "errorBkgColor",
        Value::String("#552222".to_string()),
    );
    set_if_missing(
        &mut tv,
        "errorTextColor",
        Value::String("#552222".to_string()),
    );

    let Some(primary_color) = get_truthy_string(&tv, "primaryColor") else {
        config.set_value("themeVariables", Value::Object(tv));
        return;
    };
    let Some(primary_rgb) = parse_hex_rgb01(&primary_color) else {
        config.set_value("themeVariables", Value::Object(tv));
        return;
    };
    let primary_hsl = rgb01_to_hsl(primary_rgb);
    if get_truthy_string(&tv, "primaryTextColor").is_none() {
        tv.insert(
            "primaryTextColor".to_string(),
            Value::String(invert_rgb01_to_hex(primary_rgb)),
        );
    }

    let secondary_color =
        get_truthy_string(&tv, "secondaryColor").unwrap_or_else(|| "#cdffb2".to_string());
    let secondary_hsl = parse_hex_rgb01(&secondary_color)
        .map(rgb01_to_hsl)
        .unwrap_or(primary_hsl);

    // `theme-forest` diagram-facing surfaces.
    // Source: `theme-forest.js` constructor + `updateColors()`.
    set_if_missing(&mut tv, "mainBkg", Value::String(primary_color.clone()));
    set_if_missing(&mut tv, "secondBkg", Value::String(secondary_color.clone()));
    // Table striping colors (used by ER diagrams).
    // Source: `theme-forest.js`:
    //   rowOdd  = lighten(mainBkg, 75) || '#ffffff'
    //   rowEven = lighten(mainBkg, 20)
    set_if_missing(
        &mut tv,
        "rowOdd",
        Value::String(fmt_hsl(adjust_hsl(primary_hsl, 0.0, 0.0, 75.0))),
    );
    set_if_missing(
        &mut tv,
        "rowEven",
        Value::String(fmt_hsl(adjust_hsl(primary_hsl, 0.0, 0.0, 20.0))),
    );

    // `invert('white')` in `khroma` ends up as a pure black in Mermaid's serialized SVG output.
    set_if_missing(&mut tv, "lineColor", Value::String("#000000".to_string()));
    set_if_missing(&mut tv, "textColor", Value::String("#000000".to_string()));

    // Flowchart variables (after `updateColors()`).
    set_if_missing(&mut tv, "nodeBkg", Value::String(primary_color.clone()));
    set_if_missing(&mut tv, "nodeBorder", Value::String("#13540c".to_string()));
    set_if_missing(
        &mut tv,
        "clusterBkg",
        Value::String(secondary_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "clusterBorder",
        Value::String("#6eaa49".to_string()),
    );
    set_if_missing(
        &mut tv,
        "defaultLinkColor",
        Value::String("#000000".to_string()),
    );

    // mkBorder(...) helper (shared across themes).
    let dark_mode = tv
        .get("darkMode")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    set_if_missing(
        &mut tv,
        "primaryBorderColor",
        Value::String(mk_border_hsl(primary_hsl, dark_mode)),
    );
    set_if_missing(
        &mut tv,
        "secondaryBorderColor",
        Value::String(mk_border_hsl(secondary_hsl, dark_mode)),
    );
    ensure_gradient_theme_defaults(&mut tv);

    // `theme-forest` sets: `tertiaryColor = lighten(primaryColor, 10)`.
    let tertiary_hsl = if let Some(v) =
        get_truthy_string(&tv, "tertiaryColor").and_then(|s| parse_hex_rgb01(&s).map(rgb01_to_hsl))
    {
        v
    } else {
        adjust_hsl(primary_hsl, 0.0, 0.0, 10.0)
    };
    set_if_missing(
        &mut tv,
        "tertiaryColor",
        Value::String(fmt_hsl(tertiary_hsl)),
    );
    set_if_missing(
        &mut tv,
        "tertiaryBorderColor",
        Value::String(mk_border_hsl(tertiary_hsl, dark_mode)),
    );

    // `theme-forest` ends up using black label text (via `actorTextColor`).
    set_if_missing(
        &mut tv,
        "labelTextColor",
        Value::String("black".to_string()),
    );
    set_if_missing(
        &mut tv,
        "scaleLabelColor",
        Value::String("black".to_string()),
    );
    let scale_label_color =
        get_truthy_string(&tv, "scaleLabelColor").unwrap_or_else(|| "black".to_string());

    // Color scales: match `theme-forest` `updateColors()`:
    // - derive from base colors / hue shifts
    // - darken each `cScale*` by 10
    // - `cScalePeer1/2` use special darken amounts, others are darken(`cScale*`, 25)
    let c_scales: [Hsl; 12] = [
        primary_hsl,
        secondary_hsl,
        tertiary_hsl,
        adjust_hsl(primary_hsl, 30.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 60.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 90.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 120.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 150.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 210.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 270.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 300.0, 0.0, 0.0),
        adjust_hsl(primary_hsl, 330.0, 0.0, 0.0),
    ]
    .map(|base| adjust_hsl(base, 0.0, 0.0, -10.0));

    for (i, v) in c_scales.iter().enumerate() {
        set_if_missing(&mut tv, &format!("cScale{i}"), Value::String(fmt_hsl(*v)));
    }

    set_if_missing(
        &mut tv,
        "cScalePeer1",
        Value::String(fmt_hsl(adjust_hsl(secondary_hsl, 0.0, 0.0, -45.0))),
    );
    set_if_missing(
        &mut tv,
        "cScalePeer2",
        Value::String(fmt_hsl(adjust_hsl(tertiary_hsl, 0.0, 0.0, -40.0))),
    );

    for (i, c_hsl) in c_scales.iter().enumerate() {
        set_if_missing(
            &mut tv,
            &format!("cScalePeer{i}"),
            Value::String(fmt_hsl(adjust_hsl(*c_hsl, 0.0, 0.0, -25.0))),
        );
        set_if_missing(
            &mut tv,
            &format!("cScaleInv{i}"),
            Value::String(fmt_hsl(adjust_hsl(*c_hsl, 180.0, 0.0, 0.0))),
        );
        set_if_missing(
            &mut tv,
            &format!("cScaleLabel{i}"),
            Value::String(scale_label_color.clone()),
        );
    }

    // `theme-forest` xychart palette + colors.
    // Source: `theme-forest.js`.
    ensure_xychart_theme_defaults(
        &mut tv,
        "#CDE498,#FF6B6B,#A0D2DB,#D7BDE2,#F0F0F0,#FFC3A0,#7FD8BE,#FF9A8B,#FAF3E0,#FFF176",
    );

    config.set_value("themeVariables", Value::Object(tv));
}

fn apply_neutral_theme_defaults(config: &mut MermaidConfig) {
    let mut tv = theme_variables_map(config);

    // `theme-neutral` constructor defaults.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-neutral.js`.
    set_if_missing(&mut tv, "background", Value::String("#ffffff".to_string()));
    set_if_missing(&mut tv, "primaryColor", Value::String("#eee".to_string()));
    if get_truthy_string(&tv, "primaryTextColor").is_none() {
        if let Some(primary_color) = get_truthy_string(&tv, "primaryColor") {
            if let Some(rgb) = parse_hex_rgb01(&primary_color) {
                tv.insert(
                    "primaryTextColor".to_string(),
                    Value::String(invert_rgb01_to_hex(rgb)),
                );
            }
        }
    }

    // Mermaid 11.12.2: `theme-neutral` color scale seeds.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-neutral.js`.
    let c_scales_hex: [&str; 12] = [
        "#555", "#F4F4F4", "#555", "#BBB", "#777", "#999", "#DDD", "#FFF", "#DDD", "#BBB", "#999",
        "#777",
    ];

    let primary_color =
        get_truthy_string(&tv, "primaryColor").unwrap_or_else(|| "#eee".to_string());
    let primary_hsl = parse_hex_rgb01(&primary_color)
        .map(rgb01_to_hsl)
        .unwrap_or(Hsl {
            h_deg: 0.0,
            s_pct: 0.0,
            l_pct: 93.3333333333,
        });
    let contrast = get_truthy_string(&tv, "contrast").unwrap_or_else(|| "#707070".to_string());
    let secondary_hsl = parse_hex_rgb01(&contrast)
        .map(rgb01_to_hsl)
        .map(|hsl| adjust_hsl(hsl, 0.0, 0.0, 55.0))
        .unwrap_or(Hsl {
            h_deg: 0.0,
            s_pct: 0.0,
            l_pct: 98.9215686275,
        });
    set_if_missing(
        &mut tv,
        "secondaryColor",
        Value::String(fmt_hsl(secondary_hsl)),
    );
    set_if_missing(
        &mut tv,
        "primaryBorderColor",
        Value::String(mk_border_hsl(primary_hsl, false)),
    );
    set_if_missing(
        &mut tv,
        "secondaryBorderColor",
        Value::String(mk_border_hsl(secondary_hsl, false)),
    );
    ensure_gradient_theme_defaults(&mut tv);

    set_if_missing(&mut tv, "labelTextColor", Value::String("#333".to_string()));
    set_if_missing(
        &mut tv,
        "scaleLabelColor",
        Value::String("#333".to_string()),
    );
    let scale_label_color =
        get_truthy_string(&tv, "scaleLabelColor").unwrap_or_else(|| "#333".to_string());

    for (i, c_hex) in c_scales_hex.iter().enumerate() {
        set_if_missing(
            &mut tv,
            &format!("cScale{i}"),
            Value::String((*c_hex).to_string()),
        );

        let Some(rgb) = parse_hex_rgb01(c_hex) else {
            continue;
        };
        let hsl = rgb01_to_hsl(rgb);

        // `theme-neutral` peers: `darken(cScale, 10)` (darkMode defaults to false).
        set_if_missing(
            &mut tv,
            &format!("cScalePeer{i}"),
            Value::String(fmt_hsl(adjust_hsl(hsl, 0.0, 0.0, -10.0))),
        );

        // `theme-neutral` inverted scale: `invert(cScale)`.
        set_if_missing(
            &mut tv,
            &format!("cScaleInv{i}"),
            Value::String(invert_rgb01_to_hex(rgb)),
        );

        // `theme-neutral` label scale: `scaleLabelColor`, with special-cased indices.
        // - `cScaleLabel0` and `cScaleLabel2`: `cScale1` (light fill needs dark text)
        if i == 0 || i == 2 {
            set_if_missing(
                &mut tv,
                &format!("cScaleLabel{i}"),
                Value::String(c_scales_hex[1].to_string()),
            );
        }
        set_if_missing(
            &mut tv,
            &format!("cScaleLabel{i}"),
            Value::String(scale_label_color.clone()),
        );
    }

    // `theme-neutral` xychart palette + colors.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-neutral.js`.
    ensure_xychart_theme_defaults(
        &mut tv,
        "#EEE,#6BB8E4,#8ACB88,#C7ACD6,#E8DCC2,#FFB2A8,#FFF380,#7E8D91,#FFD8B1,#FAF3E0",
    );

    config.set_value("themeVariables", Value::Object(tv));
}

fn apply_base_theme_defaults(config: &mut MermaidConfig) {
    let mut tv = theme_variables_map(config);

    let dark_mode = tv
        .get("darkMode")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let background = get_truthy_string(&tv, "background").unwrap_or_else(|| "#f4f4f4".to_string());
    let primary_color =
        get_truthy_string(&tv, "primaryColor").unwrap_or_else(|| "#fff4dd".to_string());

    // `theme-base` constructor defaults.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-base.js`.
    set_if_missing(&mut tv, "background", Value::String(background.clone()));
    set_if_missing(
        &mut tv,
        "primaryColor",
        Value::String(primary_color.clone()),
    );

    set_if_missing(
        &mut tv,
        "primaryTextColor",
        Value::String(if dark_mode { "#eee" } else { "#333" }.to_string()),
    );
    set_if_missing(&mut tv, "fontFamily", mermaid_default_font_family());
    set_if_missing(&mut tv, "fontSize", Value::String("16px".to_string()));

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
        Some(mk_border_hsl(primary_hsl, dark_mode))
    };
    if let Some(color) = primary_border_hsl {
        tv.insert("primaryBorderColor".to_string(), Value::String(color));
    }

    let secondary_border_hsl = if get_truthy_string(&tv, "secondaryBorderColor").is_some() {
        None
    } else {
        Some(mk_border_hsl(secondary_hsl, dark_mode))
    };
    if let Some(color) = secondary_border_hsl {
        tv.insert("secondaryBorderColor".to_string(), Value::String(color));
    }

    let tertiary_border_hsl = if get_truthy_string(&tv, "tertiaryBorderColor").is_some() {
        None
    } else {
        Some(mk_border_hsl(tertiary_hsl, dark_mode))
    };
    if let Some(color) = tertiary_border_hsl {
        tv.insert("tertiaryBorderColor".to_string(), Value::String(color));
    }

    if get_truthy_string(&tv, "lineColor").is_none() {
        if let Some(bg_rgb) = parse_hex_rgb01(&background) {
            tv.insert(
                "lineColor".to_string(),
                Value::String(invert_rgb01_to_hex(bg_rgb)),
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
    ensure_gradient_theme_defaults(&mut tv);

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

    // Theme color scales (used across multiple diagrams, including radar's `cScale*` palette).
    // Mermaid's base theme derives these from `primaryColor` and then darkens them.
    let darken_amount = if dark_mode { 75.0 } else { 25.0 };
    for (key, base) in [
        ("cScale0", primary_hsl),
        ("cScale1", secondary_hsl),
        ("cScale2", tertiary_hsl),
        ("cScale3", adjust_hsl(primary_hsl, 30.0, 0.0, 0.0)),
        ("cScale4", adjust_hsl(primary_hsl, 60.0, 0.0, 0.0)),
        ("cScale5", adjust_hsl(primary_hsl, 90.0, 0.0, 0.0)),
        ("cScale6", adjust_hsl(primary_hsl, 120.0, 0.0, 0.0)),
        ("cScale7", adjust_hsl(primary_hsl, 150.0, 0.0, 0.0)),
        ("cScale8", adjust_hsl(primary_hsl, 210.0, 0.0, 150.0)),
        ("cScale9", adjust_hsl(primary_hsl, 270.0, 0.0, 0.0)),
        ("cScale10", adjust_hsl(primary_hsl, 300.0, 0.0, 0.0)),
        ("cScale11", adjust_hsl(primary_hsl, 330.0, 0.0, 0.0)),
    ] {
        let v = adjust_hsl(base, 0.0, 0.0, -darken_amount);
        set_if_missing(&mut tv, key, Value::String(fmt_hsl(v)));
    }

    // Diagram style defaults (themeVariables.radar.*).
    let mut radar = match tv.get("radar") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    let line_color = get_truthy_string(&tv, "lineColor").unwrap_or_else(|| "#333333".to_string());
    set_if_missing(&mut radar, "axisColor", Value::String(line_color));
    set_if_missing(&mut radar, "axisStrokeWidth", Value::Number(2.into()));
    set_if_missing(&mut radar, "axisLabelFontSize", Value::Number(12.into()));
    set_if_missing(
        &mut radar,
        "curveOpacity",
        Value::Number(serde_json::Number::from_f64(0.5).unwrap()),
    );
    set_if_missing(&mut radar, "curveStrokeWidth", Value::Number(2.into()));
    set_if_missing(
        &mut radar,
        "graticuleColor",
        Value::String("#DEDEDE".to_string()),
    );
    set_if_missing(&mut radar, "graticuleStrokeWidth", Value::Number(1.into()));
    set_if_missing(
        &mut radar,
        "graticuleOpacity",
        Value::Number(serde_json::Number::from_f64(0.3).unwrap()),
    );
    set_if_missing(&mut radar, "legendBoxSize", Value::Number(12.into()));
    set_if_missing(&mut radar, "legendFontSize", Value::Number(12.into()));
    tv.insert("radar".to_string(), Value::Object(radar));

    // `theme-base` xychart palette + colors.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-base.js`.
    ensure_xychart_theme_defaults(
        &mut tv,
        "#FFF4DD,#FFD8B1,#FFA07A,#ECEFF1,#D6DBDF,#C3E0A8,#FFB6A4,#FFD74D,#738FA7,#FFFFF0",
    );

    config.set_value("themeVariables", Value::Object(tv));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn supported_theme_names_match_core_expansion_surface() {
        assert_eq!(
            crate::supported_themes(),
            &["default", "base", "dark", "forest", "neutral"]
        );
    }

    #[test]
    fn default_theme_populates_mermaid_theme_variables() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "default"
        }));
        apply_theme_defaults(&mut cfg);

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("background").and_then(|v| v.as_str()), Some("white"));
        assert_eq!(
            tv.get("primaryColor").and_then(|v| v.as_str()),
            Some("#ECECFF")
        );
        assert_eq!(
            tv.get("secondaryColor").and_then(|v| v.as_str()),
            Some("#ffffde")
        );
        assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#ECECFF"));
        assert_eq!(
            tv.get("nodeBorder").and_then(|v| v.as_str()),
            Some("#9370DB")
        );
        assert_eq!(
            tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
            Some("rgba(232,232,232, 0.8)")
        );
        assert_eq!(
            tv.get("classText").and_then(|v| v.as_str()),
            Some("#131300")
        );
        assert_eq!(
            tv.get("noteTextColor").and_then(|v| v.as_str()),
            Some("black")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("hsl(240, 60%, 86.2745098039%)")
        );

        let xy = tv.get("xyChart").and_then(|v| v.as_object()).unwrap();
        assert_eq!(
            xy.get("backgroundColor").and_then(|v| v.as_str()),
            Some("white")
        );
        assert_eq!(
            xy.get("dataLabelColor").and_then(|v| v.as_str()),
            Some("#131300")
        );
    }

    #[test]
    fn default_theme_preserves_user_overrides_after_derivation() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "default",
            "themeVariables": {
                "primaryColor": "#111111",
                "mainBkg": "#101010",
                "classText": "#abcdef",
                "xyChart": {
                    "titleColor": "red"
                }
            }
        }));
        apply_theme_defaults(&mut cfg);

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(
            tv.get("primaryColor").and_then(|v| v.as_str()),
            Some("#111111")
        );
        assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#101010"));
        assert_eq!(tv.get("nodeBkg").and_then(|v| v.as_str()), Some("#101010"));
        assert_eq!(
            tv.get("classText").and_then(|v| v.as_str()),
            Some("#abcdef")
        );
        assert_eq!(
            tv.get("primaryTextColor").and_then(|v| v.as_str()),
            Some("#131300")
        );

        let xy = tv.get("xyChart").and_then(|v| v.as_object()).unwrap();
        assert_eq!(xy.get("titleColor").and_then(|v| v.as_str()), Some("red"));
        assert_eq!(
            xy.get("dataLabelColor").and_then(|v| v.as_str()),
            Some("#131300")
        );
    }

    #[test]
    fn unknown_theme_falls_back_to_default_theme_variables() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "unknown"
        }));
        apply_theme_defaults(&mut cfg);

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(
            tv.get("primaryColor").and_then(|v| v.as_str()),
            Some("#ECECFF")
        );
        assert_eq!(
            tv.get("classText").and_then(|v| v.as_str()),
            Some("#131300")
        );
    }

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
        assert_eq!(
            tv.get("secondaryBorderColor").and_then(|v| v.as_str()),
            Some("hsl(164.0816326531, 5.7943925234%, 30.9803921569%)")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("hsl(284.0816326531, 5.7943925234%, 30.9803921569%)")
        );
        assert_eq!(
            tv.get("gradientStop").and_then(|v| v.as_str()),
            Some("hsl(164.0816326531, 5.7943925234%, 30.9803921569%)")
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

    #[test]
    fn forest_theme_derives_cscale_palette_like_upstream() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "forest"
        }));
        apply_theme_defaults(&mut cfg);

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(
            tv.get("cScale0").and_then(|v| v.as_str()),
            Some("hsl(78.1578947368, 58.4615384615%, 64.5098039216%)")
        );
        assert_eq!(
            tv.get("cScalePeer0").and_then(|v| v.as_str()),
            Some("hsl(78.1578947368, 58.4615384615%, 39.5098039216%)")
        );
        assert_eq!(
            tv.get("cScalePeer1").and_then(|v| v.as_str()),
            Some("hsl(98.961038961, 100%, 39.9019607843%)")
        );
        assert_eq!(
            tv.get("cScalePeer2").and_then(|v| v.as_str()),
            Some("hsl(78.1578947368, 58.4615384615%, 44.5098039216%)")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("hsl(78.1578947368, 18.4615384615%, 64.5098039216%)")
        );
        assert_eq!(
            tv.get("gradientStop").and_then(|v| v.as_str()),
            Some("hsl(98.961038961, 60%, 74.9019607843%)")
        );
    }

    #[test]
    fn dark_theme_derives_peer_and_inverted_scales_like_upstream() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "dark"
        }));
        apply_theme_defaults(&mut cfg);

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("cScale1").and_then(|v| v.as_str()), Some("#0b0000"));
        assert_eq!(
            tv.get("cScalePeer1").and_then(|v| v.as_str()),
            Some("hsl(0, 100%, 12.1568627451%)")
        );
        assert_eq!(
            tv.get("cScaleInv1").and_then(|v| v.as_str()),
            Some("#f4ffff")
        );
        assert_eq!(
            tv.get("cScaleLabel1").and_then(|v| v.as_str()),
            Some("lightgrey")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("#cccccc")
        );
    }

    #[test]
    fn neutral_theme_derives_peer_and_label_scales_like_upstream() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "neutral"
        }));
        apply_theme_defaults(&mut cfg);

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("cScale0").and_then(|v| v.as_str()), Some("#555"));
        assert_eq!(
            tv.get("cScalePeer0").and_then(|v| v.as_str()),
            Some("hsl(0, 0%, 23.3333333333%)")
        );
        assert_eq!(
            tv.get("cScaleInv0").and_then(|v| v.as_str()),
            Some("#aaaaaa")
        );
        assert_eq!(
            tv.get("cScaleLabel0").and_then(|v| v.as_str()),
            Some("#F4F4F4")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("hsl(0, 0%, 83.3333333333%)")
        );
        assert_eq!(
            tv.get("gradientStop").and_then(|v| v.as_str()),
            Some("hsl(0, 0%, 88.9215686275%)")
        );
    }
}
