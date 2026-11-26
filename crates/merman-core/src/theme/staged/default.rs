//! Compatibility calculation for the classic Default theme.
//!
//! The reference is Mermaid 12.3.0 `packages/mermaid/src/themes/theme-default.js`.
//! This update preserves Merman's fixed palette bounds and snapshot-based defaults.

use super::super::{
    ColorTransform, THEME_SCALE_INVERSE_KEYS, THEME_SCALE_KEYS, THEME_SCALE_LABEL_KEYS,
    THEME_SCALE_PEER_KEYS, ThemeProgram, XYCHART_TEXT_COLOR_KEYS, apply_quadrant_theme_defaults,
    generated_theme_runtime, get_truthy_string, merge_theme_variable_defaults, mk_border,
    required_color, set_derived_string_unless_explicit, set_if_missing,
    validate_explicit_git_transforms,
};
use super::{ColorAdjustment, ColorError, Map, ThemeState, Value, copy, theme_color};

// Each copy reads the calculated value left by the preceding assignments.
const DEPENDENCIES: &[(&str, &str)] = &[
    ("border1", "nodeBorder"),
    ("textColor", "titleColor"),
    ("labelBackground", "edgeLabelBackground"),
    ("border1", "actorBorder"),
    ("mainBkg", "actorBkg"),
    ("actorBorder", "actorLineColor"),
    ("actorBkg", "labelBoxBkgColor"),
    ("textColor", "signalColor"),
    ("textColor", "signalTextColor"),
    ("actorBorder", "labelBoxBorderColor"),
    ("actorTextColor", "labelTextColor"),
    ("actorTextColor", "loopTextColor"),
    ("border2", "noteBorderColor"),
    ("actorTextColor", "noteTextColor"),
    ("nodeBorder", "innerEndBackground"),
    ("lineColor", "specialStateColor"),
    ("lineColor", "archEdgeColor"),
    ("lineColor", "archEdgeArrowColor"),
];

pub(super) fn update(
    stage: &mut ThemeState,
    theme: &ThemeProgram,
    explicit: &Map<String, Value>,
) -> Result<(), ColorError> {
    calculate(&mut stage.variables, explicit)?;
    merge_theme_variable_defaults(&mut stage.variables, theme.calculation_snapshot(explicit));
    update_rows(explicit, &mut stage.variables)?;
    for &(source, target) in DEPENDENCIES {
        stage.assign(target, |stage| copy(stage, source))?;
    }
    Ok(())
}

// Merman restores the light Default palette before replay. Explicit slots still win at replay.
pub(super) fn restore_baseline_palette(
    target: &mut Map<String, Value>,
    baseline: &Map<String, Value>,
) {
    for keys in [
        THEME_SCALE_KEYS.as_slice(),
        THEME_SCALE_PEER_KEYS.as_slice(),
        THEME_SCALE_INVERSE_KEYS.as_slice(),
        THEME_SCALE_LABEL_KEYS.as_slice(),
        THEME_SURFACE_KEYS.as_slice(),
        THEME_SURFACE_PEER_KEYS.as_slice(),
        THEME_PIE_KEYS.as_slice(),
        &["scaleLabelColor"],
    ] {
        for &key in keys {
            if let Some(value) = baseline.get(key) {
                target.insert(key.to_string(), value.clone());
            }
        }
    }
}

const THEME_SURFACE_KEYS: [&str; 12] = [
    "surface0",
    "surface1",
    "surface2",
    "surface3",
    "surface4",
    "surface5",
    "surface6",
    "surface7",
    "surface8",
    "surface9",
    "surface10",
    "surface11",
];
const THEME_SURFACE_PEER_KEYS: [&str; 12] = [
    "surfacePeer0",
    "surfacePeer1",
    "surfacePeer2",
    "surfacePeer3",
    "surfacePeer4",
    "surfacePeer5",
    "surfacePeer6",
    "surfacePeer7",
    "surfacePeer8",
    "surfacePeer9",
    "surfacePeer10",
    "surfacePeer11",
];
const THEME_PIE_KEYS: [&str; 12] = [
    "pie1", "pie2", "pie3", "pie4", "pie5", "pie6", "pie7", "pie8", "pie9", "pie10", "pie11",
    "pie12",
];

fn update_rows(
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let primary = required_color(tv, "primaryColor")?;
    set_derived_string_unless_explicit(
        tv,
        explicit,
        "rowOdd",
        theme_color::lighten(&primary, 75.0)?,
    );
    set_derived_string_unless_explicit(
        tv,
        explicit,
        "rowEven",
        theme_color::lighten(&primary, 1.0)?,
    );
    Ok(())
}

fn ensure_xychart_theme(tv: &mut Map<String, Value>) {
    let constructor = generated_theme_runtime()
        .prepared_constructors
        .default
        .xy_chart
        .resolved();
    // An absent object keeps constructor snapshots. An authored object replaces that snapshot
    // before updateColors derives missing fields; explicit replay restores its authored leaves.
    let mut xy = match tv.get("xyChart") {
        Some(Value::Object(m)) => m.clone(),
        Some(_) => Map::new(),
        None => constructor.clone(),
    };

    set_if_missing(
        &mut xy,
        "backgroundColor",
        tv.get("background")
            .cloned()
            .unwrap_or_else(|| Value::String("white".into())),
    );
    let primary_text = tv
        .get("primaryTextColor")
        .cloned()
        .unwrap_or_else(|| Value::String("#131300".into()));
    for key in XYCHART_TEXT_COLOR_KEYS {
        set_if_missing(&mut xy, key, primary_text.clone());
    }
    set_if_missing(
        &mut xy,
        "plotColorPalette",
        constructor["plotColorPalette"].clone(),
    );

    tv.insert("xyChart".to_string(), Value::Object(xy));
}

fn mermaid_default_font_family() -> Value {
    Value::String("\"trebuchet ms\", verdana, arial, sans-serif".to_string())
}

fn calculate(tv: &mut Map<String, Value>, explicit: &Map<String, Value>) -> Result<(), ColorError> {
    // Default initializes missing roots after the overlay, before using the calculated snapshot.
    let default_primary = "#ECECFF";
    let default_secondary = "#ffffde";
    let default_tertiary =
        theme_color::adjust(default_primary, ColorAdjustment::hsl(-160.0, 0.0, 0.0))?;
    let default_primary_border = mk_border(default_primary, false)?;
    let default_secondary_border = mk_border(default_secondary, false)?;

    set_if_missing(tv, "background", Value::String("white".to_string()));
    set_if_missing(
        tv,
        "primaryColor",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        tv,
        "secondaryColor",
        Value::String(default_secondary.to_string()),
    );
    set_if_missing(tv, "tertiaryColor", Value::String(default_tertiary.clone()));

    set_if_missing(
        tv,
        "primaryBorderColor",
        Value::String(default_primary_border.clone()),
    );
    set_if_missing(
        tv,
        "secondaryBorderColor",
        Value::String(default_secondary_border.clone()),
    );
    set_if_missing(
        tv,
        "tertiaryBorderColor",
        Value::String(mk_border(&default_tertiary, false)?),
    );

    set_if_missing(tv, "primaryTextColor", Value::String("#131300".to_string()));
    set_if_missing(
        tv,
        "secondaryTextColor",
        Value::String("#000021".to_string()),
    );
    set_if_missing(
        tv,
        "tertiaryTextColor",
        Value::String(theme_color::invert(&default_tertiary)?),
    );

    set_if_missing(tv, "mainBkg", Value::String(default_primary.to_string()));
    set_if_missing(
        tv,
        "secondBkg",
        Value::String(default_secondary.to_string()),
    );
    set_if_missing(tv, "lineColor", Value::String("#333333".to_string()));
    set_if_missing(tv, "border1", Value::String("#9370DB".to_string()));
    set_if_missing(tv, "border2", Value::String("#aaaa33".to_string()));
    set_if_missing(tv, "arrowheadColor", Value::String("#333333".to_string()));
    set_if_missing(tv, "fontFamily", mermaid_default_font_family());
    set_if_missing(tv, "fontSize", Value::String("16px".to_string()));
    set_if_missing(
        tv,
        "labelBackground",
        Value::String("rgba(232,232,232, 0.8)".to_string()),
    );
    set_if_missing(tv, "textColor", Value::String("#333".to_string()));
    set_if_missing(tv, "THEME_COLOR_LIMIT", Value::Number(12.into()));
    set_if_missing(tv, "radius", Value::Number(5.into()));
    set_if_missing(tv, "strokeWidth", Value::Number(1.into()));

    let main_bkg = get_truthy_string(tv, "mainBkg").unwrap_or_else(|| default_primary.to_string());
    let second_bkg =
        get_truthy_string(tv, "secondBkg").unwrap_or_else(|| default_secondary.to_string());
    let line_color = get_truthy_string(tv, "lineColor").unwrap_or_else(|| "#333333".to_string());
    let text_color = get_truthy_string(tv, "textColor").unwrap_or_else(|| "#333".to_string());
    let border1 = get_truthy_string(tv, "border1").unwrap_or_else(|| "#9370DB".to_string());
    let border2 = get_truthy_string(tv, "border2").unwrap_or_else(|| "#aaaa33".to_string());
    let label_background = get_truthy_string(tv, "labelBackground")
        .unwrap_or_else(|| "rgba(232,232,232, 0.8)".to_string());
    let primary_text_color =
        get_truthy_string(tv, "primaryTextColor").unwrap_or_else(|| "#131300".to_string());

    // Flowchart and block/class surfaces.
    set_if_missing(tv, "nodeBkg", Value::String(main_bkg.clone()));
    set_if_missing(tv, "nodeBorder", Value::String(border1.clone()));
    set_if_missing(tv, "clusterBkg", Value::String(second_bkg.clone()));
    set_if_missing(tv, "clusterBorder", Value::String(border2.clone()));
    set_if_missing(tv, "defaultLinkColor", Value::String(line_color.clone()));
    set_if_missing(tv, "titleColor", Value::String(text_color.clone()));
    set_if_missing(
        tv,
        "edgeLabelBackground",
        Value::String(label_background.clone()),
    );
    // Sequence diagram surfaces.
    set_if_missing(tv, "actorBorder", Value::String(border1.clone()));
    set_if_missing(tv, "actorBkg", Value::String(main_bkg.clone()));
    set_if_missing(tv, "actorTextColor", Value::String("black".to_string()));
    let actor_text_color =
        get_truthy_string(tv, "actorTextColor").unwrap_or_else(|| "black".to_string());
    set_if_missing(tv, "actorLineColor", Value::String(border1.clone()));
    set_if_missing(tv, "labelBoxBkgColor", Value::String(main_bkg.clone()));
    set_if_missing(tv, "signalColor", Value::String(text_color.clone()));
    set_if_missing(tv, "signalTextColor", Value::String(text_color.clone()));
    set_if_missing(tv, "labelBoxBorderColor", Value::String(border1.clone()));
    set_if_missing(
        tv,
        "labelTextColor",
        Value::String(actor_text_color.clone()),
    );
    set_if_missing(tv, "loopTextColor", Value::String(actor_text_color.clone()));
    set_if_missing(tv, "noteBorderColor", Value::String(border2.clone()));
    set_if_missing(tv, "noteBkgColor", Value::String("#fff5ad".to_string()));
    set_if_missing(tv, "noteTextColor", Value::String(actor_text_color.clone()));
    set_if_missing(
        tv,
        "activationBorderColor",
        Value::String("#666".to_string()),
    );
    set_if_missing(
        tv,
        "activationBkgColor",
        Value::String("#f4f4f4".to_string()),
    );
    set_if_missing(
        tv,
        "sequenceNumberColor",
        Value::String("white".to_string()),
    );
    set_if_missing(tv, "rectBkgColor", Value::String(default_tertiary.clone()));

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
        set_if_missing(tv, key, Value::String(value.to_string()));
    }

    // C4 and architecture defaults.
    let primary_border_color = match get_truthy_string(tv, "primaryBorderColor") {
        Some(color) => color,
        None => mk_border(default_primary, false)?,
    };
    set_if_missing(
        tv,
        "personBorder",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(tv, "personBkg", Value::String(main_bkg.clone()));
    set_if_missing(tv, "archEdgeColor", Value::String(line_color.clone()));
    set_if_missing(tv, "archEdgeArrowColor", Value::String(line_color.clone()));
    set_if_missing(tv, "archEdgeWidth", Value::String("3".to_string()));
    set_if_missing(tv, "archGroupBorderWidth", Value::String("2px".to_string()));

    // ER, state, class, and requirement surfaces.
    set_if_missing(
        tv,
        "rowOdd",
        Value::String(theme_color::lighten(default_primary, 75.0)?),
    );
    set_if_missing(
        tv,
        "rowEven",
        Value::String(theme_color::lighten(default_primary, 1.0)?),
    );
    set_if_missing(
        tv,
        "attributeBackgroundColorOdd",
        Value::String("#ffffff".to_string()),
    );
    set_if_missing(
        tv,
        "attributeBackgroundColorEven",
        Value::String("#f2f2f2".to_string()),
    );
    set_if_missing(tv, "labelColor", Value::String("black".to_string()));
    set_if_missing(tv, "errorBkgColor", Value::String("#552222".to_string()));
    set_if_missing(tv, "errorTextColor", Value::String("#552222".to_string()));
    // `theme-default` materializes these `||` State fallbacks during construction, before
    // calculate() applies overrides. Only the unconditional specialStateColor assignment below
    // follows a replayed lineColor override during the second updateColors() pass.
    set_if_missing(tv, "transitionColor", Value::String("#333333".to_string()));
    set_if_missing(
        tv,
        "transitionLabelColor",
        Value::String("#333".to_string()),
    );
    let state_label_color = get_truthy_string(tv, "stateBkg")
        .map(Value::String)
        .unwrap_or_else(|| Value::String(primary_text_color.clone()));
    set_if_missing(tv, "stateLabelColor", state_label_color);
    set_if_missing(tv, "stateBkg", Value::String(default_primary.to_string()));
    set_if_missing(
        tv,
        "labelBackgroundColor",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        tv,
        "compositeBackground",
        Value::String("white".to_string()),
    );
    set_if_missing(tv, "altBackground", Value::String("#f0f0f0".to_string()));
    set_if_missing(
        tv,
        "compositeTitleBackground",
        Value::String(main_bkg.clone()),
    );
    let node_border = get_truthy_string(tv, "nodeBorder").unwrap_or_else(|| border1.clone());
    set_if_missing(tv, "compositeBorder", Value::String(node_border.clone()));
    set_if_missing(tv, "innerEndBackground", Value::String(node_border));
    set_if_missing(tv, "specialStateColor", Value::String(line_color.clone()));
    set_if_missing(tv, "classText", Value::String(primary_text_color.clone()));

    // Color scale.
    let primary_color =
        get_truthy_string(tv, "primaryColor").unwrap_or_else(|| default_primary.to_string());
    let secondary_color =
        get_truthy_string(tv, "secondaryColor").unwrap_or_else(|| default_secondary.to_string());
    // The constructor's first update already materializes every tertiary-derived fallback. The
    // calculate pass therefore replays an explicit tertiary token without evaluating it.
    let tertiary_color = default_tertiary.clone();
    let c_scale_bases = [
        primary_color.clone(),
        secondary_color.clone(),
        tertiary_color.clone(),
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(30.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(60.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(90.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(120.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(150.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(210.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(270.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(300.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(330.0, 0.0, 0.0))?,
    ];
    let mut c_scales = Vec::with_capacity(c_scale_bases.len());
    for base in c_scale_bases {
        c_scales.push(theme_color::darken(&base, 10.0)?);
    }

    for (key, v) in THEME_SCALE_KEYS.iter().copied().zip(&c_scales) {
        set_if_missing(tv, key, Value::String(v.clone()));
    }
    set_if_missing(
        tv,
        "cScalePeer1",
        Value::String(theme_color::darken(&secondary_color, 45.0)?),
    );
    set_if_missing(
        tv,
        "cScalePeer2",
        Value::String(theme_color::darken(&tertiary_color, 40.0)?),
    );
    for (index, (key, fallback_color)) in
        THEME_SCALE_KEYS.iter().copied().zip(&c_scales).enumerate()
    {
        let color = get_truthy_string(tv, key).unwrap_or_else(|| fallback_color.clone());
        set_if_missing(
            tv,
            THEME_SCALE_PEER_KEYS[index],
            Value::String(theme_color::darken(&color, 25.0)?),
        );
        set_if_missing(
            tv,
            THEME_SCALE_INVERSE_KEYS[index],
            Value::String(theme_color::adjust(
                &color,
                ColorAdjustment::hsl(180.0, 0.0, 0.0),
            )?),
        );
        if index == 0 || index == 3 {
            set_if_missing(
                tv,
                THEME_SCALE_LABEL_KEYS[index],
                Value::String("#ffffff".to_string()),
            );
        }
        set_if_missing(
            tv,
            THEME_SCALE_LABEL_KEYS[index],
            Value::String(actor_text_color.clone()),
        );
    }
    set_if_missing(
        tv,
        "scaleLabelColor",
        Value::String(actor_text_color.clone()),
    );

    // Journey and pie color defaults.
    for (key, value) in [
        ("fillType0", primary_color.clone()),
        ("fillType1", secondary_color.clone()),
        (
            "fillType2",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(64.0, 0.0, 0.0))?,
        ),
        (
            "fillType3",
            theme_color::adjust(&secondary_color, ColorAdjustment::hsl(64.0, 0.0, 0.0))?,
        ),
        (
            "fillType4",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(-64.0, 0.0, 0.0))?,
        ),
        (
            "fillType5",
            theme_color::adjust(&secondary_color, ColorAdjustment::hsl(-64.0, 0.0, 0.0))?,
        ),
        (
            "fillType6",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(128.0, 0.0, 0.0))?,
        ),
        (
            "fillType7",
            theme_color::adjust(&secondary_color, ColorAdjustment::hsl(128.0, 0.0, 0.0))?,
        ),
        ("pie1", primary_color.clone()),
        ("pie2", secondary_color.clone()),
        ("pie3", theme_color::darken(&tertiary_color, 40.0)?),
        ("pie4", theme_color::darken(&primary_color, 10.0)?),
        ("pie5", theme_color::darken(&secondary_color, 30.0)?),
        ("pie6", theme_color::darken(&tertiary_color, 20.0)?),
        (
            "pie7",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(60.0, 0.0, -20.0))?,
        ),
        (
            "pie8",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(-60.0, 0.0, -40.0))?,
        ),
        (
            "pie9",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(120.0, 0.0, -40.0))?,
        ),
        (
            "pie10",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(60.0, 0.0, -40.0))?,
        ),
        (
            "pie11",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(-90.0, 0.0, -40.0))?,
        ),
        (
            "pie12",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(120.0, 0.0, -30.0))?,
        ),
    ] {
        set_if_missing(tv, key, Value::String(value));
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
        set_if_missing(tv, key, Value::String(value.to_string()));
    }

    // Requirement and git surfaces consumed by current renderers.
    set_if_missing(
        tv,
        "requirementBackground",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        tv,
        "requirementBorderColor",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(tv, "requirementBorderSize", Value::String("1".to_string()));
    set_if_missing(
        tv,
        "requirementTextColor",
        Value::String(primary_text_color.clone()),
    );
    set_if_missing(tv, "relationColor", Value::String(line_color.clone()));
    set_if_missing(
        tv,
        "relationLabelBackground",
        Value::String(label_background),
    );
    set_if_missing(
        tv,
        "relationLabelColor",
        Value::String(actor_text_color.clone()),
    );

    set_if_missing(tv, "tagLabelColor", Value::String(primary_text_color));
    set_if_missing(
        tv,
        "tagLabelBackground",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        tv,
        "tagLabelBorder",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(tv, "tagLabelFontSize", Value::String("10px".to_string()));
    set_if_missing(tv, "commitLabelColor", Value::String("#000021".to_string()));
    set_if_missing(
        tv,
        "commitLabelBackground",
        Value::String(default_secondary.to_string()),
    );
    set_if_missing(tv, "commitLabelFontSize", Value::String("10px".to_string()));

    set_if_missing(tv, "useGradient", Value::Bool(false));
    set_if_missing(
        tv,
        "gradientStart",
        Value::String(default_primary_border.clone()),
    );
    set_if_missing(tv, "gradientStop", Value::String(default_secondary_border));
    set_if_missing(
        tv,
        "dropShadow",
        Value::String("drop-shadow(1px 2px 2px rgba(185, 185, 185, 1))".to_string()),
    );

    ensure_xychart_theme(tv);

    apply_quadrant_theme_defaults(tv, default_primary, "#131300", &default_primary_border)?;
    validate_explicit_git_transforms(tv, explicit, ColorTransform::Darken(25.0))?;

    Ok(())
}
