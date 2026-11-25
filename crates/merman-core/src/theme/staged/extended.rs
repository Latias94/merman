//! Snapshot-backed stages for the six Neo and Redux themes.
//!
//! The compatibility formulas follow Mermaid 12.3.0's `theme-neo*.js` and
//! `theme-redux*.js`. Resolved snapshots supply the values that these programs do
//! not recompute. The shared executor owns admission and final explicit replay.

use super::super::{
    ColorTransform, THEME_GIT_KEYS, THEME_SCALE_INVERSE_KEYS, THEME_SCALE_KEYS,
    THEME_SCALE_LABEL_KEYS, THEME_SCALE_PEER_KEYS, apply_quadrant_theme_defaults,
    get_truthy_string, is_js_truthy, merge_theme_variable_defaults, mk_border, required_color,
    set_derived_string_unless_explicit, set_if_missing, value_is_missing,
};
use super::{ColorAdjustment, ColorError, Map, ThemeState, Value, theme_color};

mod dependencies;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Program {
    Neo,
    NeoDark,
    Redux,
    ReduxDark,
    ReduxColor,
    ReduxDarkColor,
}

impl Program {
    pub(super) const fn evaluated_color_inputs(self) -> &'static [&'static str] {
        match self {
            Self::Redux => REDUX_COLOR_INPUTS,
            _ => EXTENDED_COLOR_INPUTS,
        }
    }

    // The named palette and the independent darkMode input have different roles.
    const fn has_dark_palette(self) -> bool {
        matches!(self, Self::NeoDark | Self::ReduxDark | Self::ReduxDarkColor)
    }

    const fn is_redux_color(self) -> bool {
        matches!(self, Self::ReduxColor | Self::ReduxDarkColor)
    }
}

const EXTENDED_COLOR_INPUTS: &[&str] = &[
    "primaryColor",
    "secondaryColor",
    "tertiaryColor",
    "background",
    "cScale0",
    "cScale1",
    "git0",
    "git1",
    "quadrant1Fill",
];
const REDUX_COLOR_INPUTS: &[&str] = &[
    "primaryColor",
    "secondaryColor",
    "tertiaryColor",
    "background",
    "git0",
    "git1",
    "quadrant1Fill",
];

pub(super) fn update(
    program: Program,
    stage: &mut ThemeState,
    snapshot: &Map<String, Value>,
    explicit: &Map<String, Value>,
) -> Result<(), ColorError> {
    merge_theme_variable_defaults(&mut stage.variables, snapshot);
    apply_visible_derivations(program, explicit, &mut stage.variables, snapshot)?;
    match program {
        Program::NeoDark => apply_darkened_scale_dependencies(explicit, &mut stage.variables)?,
        Program::ReduxDark => {
            apply_darkened_scale_dependencies(explicit, &mut stage.variables)?;
            apply_dynamic_git_dependencies(explicit, &mut stage.variables)?;
        }
        Program::ReduxDarkColor => apply_dynamic_git_dependencies(explicit, &mut stage.variables)?,
        Program::Neo | Program::Redux | Program::ReduxColor => {}
    }
    dependencies::apply(program, explicit, stage)
}

const THEME_GIT_INVERSE_KEYS: [&str; 8] = [
    "gitInv0", "gitInv1", "gitInv2", "gitInv3", "gitInv4", "gitInv5", "gitInv6", "gitInv7",
];
const THEME_VENN_KEYS: [&str; 8] = [
    "venn1", "venn2", "venn3", "venn4", "venn5", "venn6", "venn7", "venn8",
];
const THEME_FILL_TYPE_KEYS: [&str; 8] = [
    "fillType0",
    "fillType1",
    "fillType2",
    "fillType3",
    "fillType4",
    "fillType5",
    "fillType6",
    "fillType7",
];

fn apply_visible_derivations(
    theme: Program,
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
    snapshot: &Map<String, Value>,
) -> Result<(), ColorError> {
    replay_khroma_operations(theme, explicit, tv)?;

    // Mermaid's extended themes run `calculate(overrides)`: copy user base variables, update
    // derived colors, then re-apply explicit user keys. Keep generated snapshots as the default
    // source of truth, but recompute visible derived keys that current renderers consume.
    if explicit.contains_key("primaryColor") {
        let primary = required_color(tv, "primaryColor")?;
        set_derived_string_unless_explicit(tv, explicit, "nodeBkg", primary.clone());
        set_derived_string_unless_explicit(tv, explicit, "tagLabelBackground", primary.clone());

        if !theme.has_dark_palette() && !explicit.contains_key("secondaryColor") {
            let secondary = theme_color::adjust(&primary, ColorAdjustment::hsl(-120.0, 0.0, 0.0))?;
            tv.insert("secondaryColor".to_string(), Value::String(secondary));
        }
    }

    // updateColors() resolves a falsy border before Agentflow reads it. The
    // explicit value is replayed only after all dependent colors are calculated.
    if value_is_missing(tv, "secondaryBorderColor") {
        let secondary = required_color(tv, "secondaryColor")?;
        let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
        tv.insert(
            "secondaryBorderColor".to_string(),
            Value::String(mk_border(&secondary, dark_mode)?),
        );
    }

    if explicit.contains_key("secondaryColor") || explicit.contains_key("primaryColor") {
        let secondary = required_color(tv, "secondaryColor")?;
        let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
        let label_background = if dark_mode {
            theme_color::darken(&secondary, 30.0)?
        } else {
            secondary.clone()
        };
        for key in [
            "edgeLabelBackground",
            "activationBkgColor",
            "commitLabelBackground",
            "relationLabelBackground",
        ] {
            set_derived_string_unless_explicit(tv, explicit, key, label_background.clone());
        }
    }

    if explicit.contains_key("mainBkg") {
        let main_bkg = required_color(tv, "mainBkg")?;
        for key in [
            "actorBkg",
            "labelBoxBkgColor",
            "personBkg",
            "stateBkg",
            "labelBackgroundColor",
        ] {
            set_derived_string_unless_explicit(tv, explicit, key, main_bkg.clone());
        }
    }

    if explicit.contains_key("primaryColor") && theme.has_dark_palette() {
        let primary = required_color(tv, "primaryColor")?;
        set_derived_string_unless_explicit(tv, explicit, "requirementBackground", primary.clone());
        if theme != Program::ReduxDarkColor {
            set_derived_string_unless_explicit(tv, explicit, "pie1", primary);
        }
    }

    // Mermaid 12 uses a visible, direction-aware band on the light extended
    // themes, and resolves dark themes' constructor placeholder from mainBkg.
    if !theme.has_dark_palette()
        && explicit.contains_key("background")
        && !explicit.contains_key("rectBkgColor")
    {
        let background = required_color(tv, "background")?;
        let band = if theme_color::is_dark(&background)? {
            theme_color::lighten(&background, 4.0)?
        } else {
            theme_color::darken(&background, 4.0)?
        };
        tv.insert("rectBkgColor".to_string(), Value::String(band));
    }
    if theme.has_dark_palette() {
        let second_is_placeholder =
            explicit.get("secondBkg").and_then(Value::as_str) == Some("calculated");
        if (explicit.contains_key("mainBkg") && !explicit.contains_key("secondBkg"))
            || second_is_placeholder
        {
            let second = theme_color::lighten(&required_color(tv, "mainBkg")?, 16.0)?;
            tv.insert("secondBkg".to_string(), Value::String(second));
        }
        if (explicit.contains_key("mainBkg") || explicit.contains_key("secondBkg"))
            && !explicit.contains_key("doneTaskBkgColor")
            && let Some(second) = tv.get("secondBkg").cloned()
        {
            tv.insert("doneTaskBkgColor".to_string(), second);
        }
    }
    if theme.is_redux_color() {
        // Resolve categorical fallbacks before deriving pie and Gantt colors.
        for scale in THEME_SCALE_KEYS.iter().copied() {
            if explicit.contains_key(scale) {
                // Both Redux color themes use fixed categorical fallbacks in
                // updateColors(). Consumers read that stage, not the final replay.
                let fallback = snapshot[scale].clone();
                set_if_missing(tv, scale, fallback);
            }
        }
        for (palette, prefix) in [("borderColorArray", "venn"), ("bkgColorArray", "fillType")] {
            if prefix == "fillType" && theme == Program::ReduxDarkColor {
                continue;
            }
            if let Some(colors) = explicit.get(palette).and_then(Value::as_array) {
                for index in 0..8 {
                    let key = if prefix == "venn" {
                        THEME_VENN_KEYS[index]
                    } else {
                        THEME_FILL_TYPE_KEYS[index]
                    };
                    if explicit.contains_key(key) {
                        continue;
                    }
                    let color = if prefix == "venn" && !colors.is_empty() {
                        colors.get(index % colors.len())
                    } else {
                        colors.get(index)
                    };
                    if let Some(color) = color {
                        tv.insert(key.to_string(), color.clone());
                    } else {
                        tv.remove(key);
                    }
                }
            }
        }
    }

    for (git_key, git_inv_key) in THEME_GIT_KEYS
        .iter()
        .copied()
        .zip(THEME_GIT_INVERSE_KEYS.iter().copied())
    {
        if explicit.contains_key(git_key) && !explicit.contains_key(git_inv_key) {
            let git = required_color(tv, git_key)?;
            let inv = theme_color::invert(&git)?;
            tv.insert(git_inv_key.to_string(), Value::String(inv));
        }
    }

    discard_non_explicit_quadrant_snapshot_values(tv, explicit);
    let quadrant_fill_base = if !theme.has_dark_palette() {
        "#ECECFE".to_string()
    } else {
        required_color(tv, "primaryColor")?
    };
    let quadrant_text_base = required_color(tv, "primaryTextColor")?;
    let quadrant_border_base = required_color(tv, "primaryBorderColor")?;
    apply_quadrant_theme_defaults(
        tv,
        &quadrant_fill_base,
        &quadrant_text_base,
        &quadrant_border_base,
    )?;
    Ok(())
}

fn apply_single_pass_git_palette(
    tv: &mut Map<String, Value>,
    explicit: &Map<String, Value>,
    bases: [String; 8],
    transform: ColorTransform,
) -> Result<(), ColorError> {
    for ((git_key, inverse_key), base) in THEME_GIT_KEYS
        .iter()
        .copied()
        .zip(THEME_GIT_INVERSE_KEYS.iter().copied())
        .zip(bases)
    {
        let source = if explicit.contains_key(git_key) {
            required_color(tv, git_key)?
        } else {
            base
        };
        let transformed = transform.apply(&source)?;
        if !explicit.contains_key(git_key) {
            tv.insert(git_key.to_string(), Value::String(transformed.clone()));
        }

        if !explicit.contains_key(inverse_key) {
            tv.insert(
                inverse_key.to_string(),
                Value::String(theme_color::invert(&transformed)?),
            );
        }
    }
    Ok(())
}

fn calculated_or_fallback_value(
    explicit: &Map<String, Value>,
    key: &str,
    fallback: Value,
) -> Value {
    explicit
        .get(key)
        .filter(|value| is_js_truthy(value))
        .cloned()
        .unwrap_or(fallback)
}

fn apply_scale_label_dependencies(
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
    fallback: Value,
) {
    let scale_label = calculated_or_fallback_value(explicit, "scaleLabelColor", fallback);
    tv.insert("scaleLabelColor".to_string(), scale_label.clone());
    for key in THEME_SCALE_LABEL_KEYS.iter().copied() {
        if !explicit.contains_key(key) {
            tv.insert(key.to_string(), scale_label.clone());
        }
    }
}

fn git_palette_bases(tv: &Map<String, Value>) -> Result<[String; 8], ColorError> {
    let primary = required_color(tv, "primaryColor")?;
    Ok([
        primary.clone(),
        required_color(tv, "secondaryColor")?,
        required_color(tv, "tertiaryColor")?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(-30.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(-60.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(-90.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(60.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(120.0, 0.0, 0.0))?,
    ])
}

fn apply_dynamic_git_dependencies(
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let transform = if tv.get("darkMode").is_some_and(is_js_truthy) {
        ColorTransform::Lighten(25.0)
    } else {
        ColorTransform::Darken(25.0)
    };
    apply_single_pass_git_palette(tv, explicit, git_palette_bases(tv)?, transform)
}

fn darkened_scale_bases(tv: &Map<String, Value>) -> Result<[String; 12], ColorError> {
    let primary = required_color(tv, "primaryColor")?;
    Ok([
        primary.clone(),
        required_color(tv, "secondaryColor")?,
        required_color(tv, "tertiaryColor")?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(30.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(60.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(90.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(120.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(150.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(210.0, 0.0, 150.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(270.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(300.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(330.0, 0.0, 0.0))?,
    ])
}

fn apply_darkened_scale_dependencies(
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
    let transform = ColorTransform::Darken(if dark_mode { 75.0 } else { 25.0 });
    let bases = darkened_scale_bases(tv)?;

    for ((scale_key, inverse_key), (peer_key, base)) in THEME_SCALE_KEYS
        .iter()
        .copied()
        .zip(THEME_SCALE_INVERSE_KEYS.iter().copied())
        .zip(THEME_SCALE_PEER_KEYS.iter().copied().zip(bases))
    {
        let source = get_truthy_string(explicit, scale_key).unwrap_or(base);
        let scale = transform.apply(&source)?;
        if !explicit.contains_key(scale_key) {
            tv.insert(scale_key.to_string(), Value::String(scale.clone()));
        }
        set_derived_string_unless_explicit(tv, explicit, inverse_key, theme_color::invert(&scale)?);
        let peer = if dark_mode {
            theme_color::lighten(&scale, 10.0)?
        } else {
            theme_color::darken(&scale, 10.0)?
        };
        set_derived_string_unless_explicit(tv, explicit, peer_key, peer);
    }

    let label_text = tv
        .get("labelTextColor")
        .cloned()
        .unwrap_or_else(|| Value::String("#e0dfdf".to_string()));
    apply_scale_label_dependencies(explicit, tv, label_text);
    Ok(())
}

fn validate_explicit_operation(
    tv: &Map<String, Value>,
    explicit: &Map<String, Value>,
    key: &str,
    operation: impl FnOnce(&str) -> Result<String, ColorError>,
) -> Result<(), ColorError> {
    if explicit.contains_key(key) {
        operation(&required_color(tv, key)?)?;
    }
    Ok(())
}

fn replay_khroma_operations(
    theme: Program,
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
    match theme {
        Program::Neo | Program::Redux | Program::ReduxColor => {
            for key in ["secondaryColor", "tertiaryColor"] {
                validate_explicit_operation(tv, explicit, key, |color| {
                    mk_border(color, dark_mode)
                })?;
            }
        }
        Program::NeoDark => {
            validate_explicit_operation(tv, explicit, "secondaryColor", |color| {
                theme_color::darken(color, 10.0)
            })?;
            validate_explicit_operation(tv, explicit, "tertiaryColor", |color| {
                theme_color::darken(color, if dark_mode { 75.0 } else { 25.0 })
            })?;
        }
        _ => {}
    }

    let scale_policy = match theme {
        Program::Neo | Program::NeoDark | Program::ReduxDark => {
            Some(Some(ColorTransform::Darken(if dark_mode {
                75.0
            } else {
                25.0
            })))
        }
        Program::ReduxColor | Program::ReduxDarkColor => Some(None),
        _ => None,
    };
    if let Some(transform) = scale_policy {
        for ((scale_key, inverse_key), (peer_key, label_key)) in THEME_SCALE_KEYS
            .into_iter()
            .zip(THEME_SCALE_INVERSE_KEYS.iter().copied())
            .zip(
                THEME_SCALE_PEER_KEYS
                    .iter()
                    .copied()
                    .zip(THEME_SCALE_LABEL_KEYS.iter().copied()),
            )
            .take(2)
        {
            if !explicit.contains_key(scale_key) {
                continue;
            }
            let source = required_color(tv, scale_key)?;
            let calculated = match transform {
                Some(transform) => transform.apply(&source)?,
                None => source,
            };

            if !explicit.contains_key(inverse_key) {
                tv.insert(
                    inverse_key.to_string(),
                    Value::String(theme_color::invert(&calculated)?),
                );
            }
            if !explicit.contains_key(peer_key) {
                let peer = if dark_mode {
                    theme_color::lighten(&calculated, 10.0)?
                } else {
                    theme_color::darken(&calculated, 10.0)?
                };
                tv.insert(peer_key.to_string(), Value::String(peer));
            }
            if theme == Program::ReduxDarkColor && !explicit.contains_key(label_key) {
                tv.insert(
                    label_key.to_string(),
                    Value::String(theme_color::darken(&calculated, 75.0)?),
                );
            }
        }
    }
    Ok(())
}

fn discard_non_explicit_quadrant_snapshot_values(
    tv: &mut Map<String, Value>,
    explicit: &Map<String, Value>,
) {
    for key in [
        "quadrant1Fill",
        "quadrant2Fill",
        "quadrant3Fill",
        "quadrant4Fill",
        "quadrant1TextFill",
        "quadrant2TextFill",
        "quadrant3TextFill",
        "quadrant4TextFill",
        "quadrantPointFill",
        "quadrantPointTextFill",
        "quadrantXAxisTextFill",
        "quadrantYAxisTextFill",
        "quadrantInternalBorderStrokeFill",
        "quadrantExternalBorderStrokeFill",
        "quadrantTitleFill",
    ] {
        if !explicit.contains_key(key) {
            tv.remove(key);
        }
    }
}
