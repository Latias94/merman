//! Mermaid 12.1.0 Base updateColors, executed once with assignment-owned provenance.
//!
//! Source: Mermaid 12.1.0 `packages/mermaid/src/themes/theme-base.js`.
//! Constructor preparation and explicit replay remain owned by the shared staged executor.

use super::*;
use crate::theme::mk_border;

fn inherit(stage: &mut ThemeState, key: &str, source: &str) -> Result<(), ColorError> {
    // The first stateLabelColor fallback may reference an absent stateBkg. JavaScript leaves
    // it undefined until the following fallback; no value or provenance exists to copy yet.
    if stage.variables.contains_key(source) {
        stage.assign_if_falsy(key, |stage| copy(stage, source))?;
    }
    Ok(())
}

fn inherit_many(stage: &mut ThemeState, pairs: &[(&str, &str)]) -> Result<(), ColorError> {
    for (key, source) in pairs {
        inherit(stage, key, source)?;
    }
    Ok(())
}

fn transform(
    stage: &mut ThemeState,
    key: &str,
    source: &str,
    operation: impl FnOnce(&str) -> Result<String, ColorError>,
) -> Result<(), ColorError> {
    stage.assign_if_falsy(key, |stage| {
        transform_color(copy(stage, source)?, operation)
    })
}

fn adjust(
    stage: &mut ThemeState,
    key: &str,
    source: &str,
    hue: f64,
    saturation: f64,
    lightness: f64,
) -> Result<(), ColorError> {
    stage.assign_if_falsy(key, |stage| {
        adjusted(
            stage,
            source,
            ColorAdjustment::hsl(hue, saturation, lightness),
        )
    })
}

fn set_string_if_missing(stage: &mut ThemeState, key: &str, value: &str) -> Result<(), ColorError> {
    stage.assign_if_falsy(key, |_| Ok(ComputedValue::string(value)))
}

pub(super) fn update(stage: &mut ThemeState) -> Result<(), ColorError> {
    let dark_mode = stage.variables.get("darkMode").is_some_and(is_js_truthy);

    stage.assign_if_falsy("primaryTextColor", |_| {
        Ok(ComputedValue::with_dependencies(
            Value::String(if dark_mode { "#eee" } else { "#333" }.to_owned()),
            ["darkMode".to_owned()],
        ))
    })?;
    adjust(stage, "secondaryColor", "primaryColor", -120.0, 0.0, 0.0)?;
    adjust(stage, "tertiaryColor", "primaryColor", 180.0, 0.0, 5.0)?;
    for (key, source) in [
        ("primaryBorderColor", "primaryColor"),
        ("secondaryBorderColor", "secondaryColor"),
        ("tertiaryBorderColor", "tertiaryColor"),
        ("noteBorderColor", "noteBkgColor"),
    ] {
        stage.assign_if_falsy(key, |stage| {
            let mut value =
                transform_color(copy(stage, source)?, |color| mk_border(color, dark_mode))?;
            value.dependencies.push("darkMode".to_owned());
            Ok(value)
        })?;
    }
    set_string_if_missing(stage, "noteBkgColor", "#fff5ad")?;
    set_string_if_missing(stage, "noteTextColor", "#333")?;
    for (key, source) in [
        ("secondaryTextColor", "secondaryColor"),
        ("tertiaryTextColor", "tertiaryColor"),
        ("lineColor", "background"),
        ("arrowheadColor", "background"),
    ] {
        transform(stage, key, source, theme_color::invert)?;
    }
    inherit_many(
        stage,
        &[
            ("textColor", "primaryTextColor"),
            ("border2", "tertiaryBorderColor"),
            ("nodeBkg", "primaryColor"),
            ("mainBkg", "primaryColor"),
            ("nodeBorder", "primaryBorderColor"),
            ("clusterBkg", "tertiaryColor"),
            ("clusterBorder", "tertiaryBorderColor"),
            ("defaultLinkColor", "lineColor"),
            ("titleColor", "tertiaryTextColor"),
        ],
    )?;
    stage.assign_if_falsy("edgeLabelBackground", |stage| {
        let mut value = if dark_mode {
            darkened(stage, "secondaryColor", 30.0)?
        } else {
            copy(stage, "secondaryColor")?
        };
        value.dependencies.push("darkMode".to_owned());
        Ok(value)
    })?;
    inherit_many(
        stage,
        &[
            ("flowContainerStroke", "secondaryBorderColor"),
            ("nodeTextColor", "primaryTextColor"),
            ("actorBorder", "primaryBorderColor"),
            ("actorBkg", "mainBkg"),
            ("actorTextColor", "primaryTextColor"),
            ("actorLineColor", "actorBorder"),
            ("labelBoxBkgColor", "actorBkg"),
            ("signalColor", "textColor"),
            ("signalTextColor", "textColor"),
            ("labelBoxBorderColor", "actorBorder"),
            ("labelTextColor", "actorTextColor"),
            ("loopTextColor", "actorTextColor"),
        ],
    )?;
    transform(stage, "activationBorderColor", "secondaryColor", |color| {
        theme_color::darken(color, 10.0)
    })?;
    inherit(stage, "activationBkgColor", "secondaryColor")?;
    transform(
        stage,
        "sequenceNumberColor",
        "lineColor",
        theme_color::invert,
    )?;
    inherit_many(
        stage,
        &[
            ("rectBkgColor", "tertiaryColor"),
            ("sectionBkgColor", "tertiaryColor"),
            ("sectionBkgColor", "secondaryColor"),
            ("sectionBkgColor2", "primaryColor"),
            ("taskBorderColor", "primaryBorderColor"),
            ("taskBkgColor", "primaryColor"),
            ("activeTaskBorderColor", "primaryColor"),
        ],
    )?;
    transform(stage, "activeTaskBkgColor", "primaryColor", |color| {
        theme_color::lighten(color, 23.0)
    })?;
    for (key, value) in [
        ("altSectionBkgColor", "white"),
        ("excludeBkgColor", "#eeeeee"),
        ("gridColor", "lightgrey"),
        ("doneTaskBkgColor", "lightgrey"),
        ("doneTaskBorderColor", "grey"),
        ("critBorderColor", "#ff8888"),
        ("critBkgColor", "red"),
        ("todayLineColor", "red"),
        ("vertLineColor", "navy"),
        ("taskTextClickableColor", "#003163"),
        ("noteFontWeight", "normal"),
        ("fontWeight", "normal"),
    ] {
        set_string_if_missing(stage, key, value)?;
    }
    inherit_many(
        stage,
        &[
            ("taskTextColor", "textColor"),
            ("taskTextOutsideColor", "textColor"),
            ("taskTextLightColor", "textColor"),
            ("taskTextColor", "primaryTextColor"),
            ("taskTextDarkColor", "textColor"),
            ("personBorder", "primaryBorderColor"),
            ("personBkg", "mainBkg"),
        ],
    )?;
    for (key, amount) in [
        ("rowOdd", if dark_mode { -5.0 } else { 75.0 }),
        ("rowEven", if dark_mode { -10.0 } else { 5.0 }),
    ] {
        stage.assign_if_falsy(key, |stage| {
            let mut value = adjusted(stage, "mainBkg", ColorAdjustment::hsl(0.0, 0.0, amount))?;
            value.dependencies.push("darkMode".to_owned());
            Ok(value)
        })?;
    }
    inherit_many(
        stage,
        &[
            ("transitionColor", "lineColor"),
            ("transitionLabelColor", "textColor"),
            // This precedes stateBkg's default assignment in upstream updateColors().
            ("stateLabelColor", "stateBkg"),
            ("stateLabelColor", "primaryTextColor"),
            ("stateBkg", "mainBkg"),
            ("labelBackgroundColor", "stateBkg"),
            ("compositeBackground", "background"),
            ("compositeBackground", "tertiaryColor"),
            ("altBackground", "tertiaryColor"),
            ("compositeTitleBackground", "mainBkg"),
            ("compositeBorder", "nodeBorder"),
            ("errorBkgColor", "tertiaryColor"),
            ("errorTextColor", "tertiaryTextColor"),
        ],
    )?;
    stage.assign("innerEndBackground", |stage| copy(stage, "nodeBorder"))?;
    stage.assign("specialStateColor", |stage| copy(stage, "lineColor"))?;

    inherit_many(
        stage,
        &[
            ("cScale0", "primaryColor"),
            ("cScale1", "secondaryColor"),
            ("cScale2", "tertiaryColor"),
        ],
    )?;
    for (index, hue) in [
        (3, 30.0),
        (4, 60.0),
        (5, 90.0),
        (6, 120.0),
        (7, 150.0),
        (8, 210.0),
        (9, 270.0),
        (10, 300.0),
        (11, 330.0),
    ] {
        adjust(
            stage,
            &format!("cScale{index}"),
            "primaryColor",
            hue,
            0.0,
            if index == 8 { 150.0 } else { 0.0 },
        )?;
    }
    for_theme_color_indices(stage, |stage, index| {
        let key = format!("cScale{index}");
        stage.assign(&key, |stage| {
            let mut value = darkened(stage, &key, if dark_mode { 75.0 } else { 25.0 })?;
            value.dependencies.push("darkMode".to_owned());
            Ok(value)
        })
    })?;
    for_theme_color_indices(stage, |stage, index| {
        let source = format!("cScale{index}");
        stage.assign_if_falsy(&format!("cScaleInv{index}"), |stage| {
            inverted(stage, &source)
        })?;
        stage.assign_if_falsy(&format!("cScalePeer{index}"), |stage| {
            let mut value = if dark_mode {
                lightened(stage, &source, 10.0)?
            } else {
                darkened(stage, &source, 10.0)?
            };
            value.dependencies.push("darkMode".to_owned());
            Ok(value)
        })
    })?;
    inherit(stage, "scaleLabelColor", "labelTextColor")?;
    for_theme_color_indices(stage, |stage, index| {
        inherit(stage, &format!("cScaleLabel{index}"), "scaleLabelColor")
    })?;
    let multiplier = if dark_mode { -4.0 } else { -1.0 };
    for index in 0..5 {
        for (key, amount) in [
            (format!("surface{index}"), 5 + index * 3),
            (format!("surfacePeer{index}"), 8 + index * 3),
        ] {
            stage.assign_if_falsy(&key, |stage| {
                let mut value = adjusted(
                    stage,
                    "mainBkg",
                    ColorAdjustment::hsl(180.0, -15.0, multiplier * amount as f64),
                )?;
                value.dependencies.push("darkMode".to_owned());
                Ok(value)
            })?;
        }
    }
    inherit_many(
        stage,
        &[
            ("classText", "textColor"),
            ("fillType0", "primaryColor"),
            ("fillType1", "secondaryColor"),
        ],
    )?;
    for (index, source, hue) in [
        (2, "primaryColor", 64.0),
        (3, "secondaryColor", 64.0),
        (4, "primaryColor", -64.0),
        (5, "secondaryColor", -64.0),
        (6, "primaryColor", 128.0),
        (7, "secondaryColor", 128.0),
    ] {
        adjust(stage, &format!("fillType{index}"), source, hue, 0.0, 0.0)?;
    }
    inherit_many(
        stage,
        &[
            ("pie1", "primaryColor"),
            ("pie2", "secondaryColor"),
            ("pie3", "tertiaryColor"),
        ],
    )?;
    for (index, source, hue, lightness) in [
        (4, "primaryColor", 0.0, -10.0),
        (5, "secondaryColor", 0.0, -10.0),
        (6, "tertiaryColor", 0.0, -10.0),
        (7, "primaryColor", 60.0, -10.0),
        (8, "primaryColor", -60.0, -10.0),
        (9, "primaryColor", 120.0, 0.0),
        (10, "primaryColor", 60.0, -20.0),
        (11, "primaryColor", -60.0, -20.0),
        (12, "primaryColor", 120.0, -10.0),
    ] {
        adjust(stage, &format!("pie{index}"), source, hue, 0.0, lightness)?;
    }
    inherit_many(
        stage,
        &[
            ("pieTitleTextColor", "taskTextDarkColor"),
            ("pieSectionTextColor", "textColor"),
            ("pieLegendTextColor", "taskTextDarkColor"),
        ],
    )?;
    for (key, value) in [
        ("pieTitleTextSize", "25px"),
        ("pieSectionTextSize", "17px"),
        ("pieLegendTextSize", "17px"),
        ("pieStrokeColor", "black"),
        ("pieStrokeWidth", "2px"),
        ("pieOuterStrokeWidth", "2px"),
        ("pieOuterStrokeColor", "black"),
        ("pieOpacity", "0.7"),
    ] {
        set_string_if_missing(stage, key, value)?;
    }
    // Venn uses nullish coalescing, unlike the truthiness fallbacks above.
    for (index, source, hue) in [
        (1, "primaryColor", 0.0),
        (2, "secondaryColor", 0.0),
        (3, "tertiaryColor", 0.0),
        (4, "primaryColor", 60.0),
        (5, "primaryColor", -60.0),
        (6, "secondaryColor", 60.0),
        (7, "primaryColor", 120.0),
        (8, "secondaryColor", 120.0),
    ] {
        let key = format!("venn{index}");
        if stage.variables.get(&key).is_none_or(Value::is_null) {
            adjust(stage, &key, source, hue, 0.0, -30.0)?;
        }
    }
    for (key, source) in [
        ("vennTitleTextColor", "titleColor"),
        ("vennSetTextColor", "textColor"),
    ] {
        if stage.variables.get(key).is_none_or(Value::is_null) {
            inherit(stage, key, source)?;
        }
    }

    update_cynefin(
        stage, "#8B0000", "#E8F5E9", "#E3F2FD", "#FBE9E7", "#FFF8E1", "#F3E5F5",
    )?;
    update_radar(stage)?;
    update_wardley(stage, "#dc3545", "background")?;
    for (key, value) in [
        ("archEdgeColor", "#777"),
        ("archEdgeArrowColor", "#777"),
        ("archEdgeWidth", "3"),
        ("archGroupBorderColor", "#000"),
        ("archGroupBorderWidth", "2px"),
    ] {
        set_string_if_missing(stage, key, value)?;
    }
    update_quadrant(stage)?;
    update_xy_chart(
        stage,
        "#FFF4DD,#FFD8B1,#FFA07A,#ECEFF1,#D6DBDF,#C3E0A8,#FFB6A4,#FFD74D,#738FA7,#FFFFF0",
    )?;
    inherit_many(
        stage,
        &[
            ("requirementBackground", "primaryColor"),
            ("requirementBorderColor", "primaryBorderColor"),
            ("requirementTextColor", "primaryTextColor"),
            ("relationColor", "lineColor"),
        ],
    )?;
    set_string_if_missing(stage, "requirementBorderSize", "1")?;
    stage.assign_if_falsy("relationLabelBackground", |stage| {
        let mut value = if dark_mode {
            darkened(stage, "secondaryColor", 30.0)?
        } else {
            copy(stage, "secondaryColor")?
        };
        value.dependencies.push("darkMode".to_owned());
        Ok(value)
    })?;
    inherit(stage, "relationLabelColor", "actorTextColor")?;
    inherit_many(
        stage,
        &[
            ("git0", "primaryColor"),
            ("git1", "secondaryColor"),
            ("git2", "tertiaryColor"),
        ],
    )?;
    for (index, hue) in [(3, -30.0), (4, -60.0), (5, -90.0), (6, 60.0), (7, 120.0)] {
        adjust(stage, &format!("git{index}"), "primaryColor", hue, 0.0, 0.0)?;
    }
    for index in 0..8 {
        let key = format!("git{index}");
        stage.assign(&key, |stage| {
            let mut value = if dark_mode {
                lightened(stage, &key, 25.0)?
            } else {
                darkened(stage, &key, 25.0)?
            };
            value.dependencies.push("darkMode".to_owned());
            Ok(value)
        })?;
    }
    for index in 0..8 {
        transform(
            stage,
            &format!("gitInv{index}"),
            &format!("git{index}"),
            theme_color::invert,
        )?;
    }
    stage.assign_if_falsy("branchLabelColor", |stage| {
        let mut value = if dark_mode {
            ComputedValue::string("black")
        } else {
            copy(stage, "labelTextColor")?
        };
        value.dependencies.push("darkMode".to_owned());
        Ok(value)
    })?;
    for index in 0..8 {
        inherit(stage, &format!("gitBranchLabel{index}"), "branchLabelColor")?;
    }
    update_tags(stage)?;
    update_event_modeling_light(stage)?;
    stage.assign("gradientStart", |stage| copy(stage, "primaryBorderColor"))?;
    stage.assign("gradientStop", |stage| copy(stage, "secondaryBorderColor"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{apply_theme_defaults, theme_variables_map};
    use serde_json::json;

    fn resolve(overrides: Value) -> Map<String, Value> {
        let mut config = MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": overrides
        }));
        apply_theme_defaults(&mut config).unwrap();
        theme_variables_map(&config)
    }

    #[test]
    fn base_overrides_reach_every_family_before_explicit_replay() {
        let tv = resolve(json!({
            "primaryColor": "#181818",
            "secondaryColor": "#224466",
            "tertiaryColor": "#aabbcc",
            "primaryTextColor": "#ddeeff",
            "lineColor": "#a0a0a0",
            "textColor": "#112233"
        }));
        for key in [
            "mainBkg",
            "nodeBkg",
            "actorBkg",
            "labelBoxBkgColor",
            "personBkg",
            "stateBkg",
            "labelBackgroundColor",
            "compositeTitleBackground",
            "taskBkgColor",
            "activeTaskBorderColor",
            "sectionBkgColor2",
            "pie1",
            "fillType0",
            "requirementBackground",
            "tagLabelBackground",
        ] {
            assert_eq!(tv[key], "#181818", "{key}");
        }
        for key in [
            "transitionColor",
            "specialStateColor",
            "defaultLinkColor",
            "relationColor",
            "emArrowhead",
            "emRelationStroke",
        ] {
            assert_eq!(tv[key], "#a0a0a0", "{key}");
        }
        for key in [
            "signalColor",
            "signalTextColor",
            "transitionLabelColor",
            "taskTextColor",
            "taskTextOutsideColor",
            "taskTextDarkColor",
            "classText",
            "pieTitleTextColor",
            "pieSectionTextColor",
            "pieLegendTextColor",
            "vennSetTextColor",
        ] {
            assert_eq!(tv[key], "#112233", "{key}");
        }
        for key in [
            "clusterBkg",
            "rectBkgColor",
            "sectionBkgColor",
            "altBackground",
            "pie3",
        ] {
            assert_eq!(tv[key], "#aabbcc", "{key}");
        }
        assert_eq!(tv["activationBkgColor"], "#224466");
        assert_eq!(tv["stateLabelColor"], "#ddeeff");
        assert_eq!(tv["arrowheadColor"], "#0b0b0b");
        assert_eq!(tv["sequenceNumberColor"], "#5f5f5f");
        assert_eq!(tv["radar"]["axisColor"], "#a0a0a0");
        assert_eq!(tv["cynefin"]["boundaryColor"], "#a0a0a0");
        assert_eq!(tv["wardley"]["linkStroke"], "#a0a0a0");
        assert_eq!(tv["xyChart"]["legendTextColor"], "#ddeeff");
    }

    #[test]
    fn base_intermediate_overrides_feed_dependents_in_upstream_order() {
        let tv = resolve(json!({
            "primaryColor": "#181818", "mainBkg": "#242424",
            "actorBkg": "#345678", "actorBorder": "#456789", "actorTextColor": "#abcdef",
            "stateBkg": "#567890", "taskTextDarkColor": "#678901", "tagBorder": "#789012"
        }));
        assert_eq!(tv["personBkg"], "#242424");
        assert_eq!(tv["compositeTitleBackground"], "#242424");
        assert_eq!(tv["labelBoxBkgColor"], "#345678");
        assert_eq!(tv["actorLineColor"], "#456789");
        assert_eq!(tv["labelBoxBorderColor"], "#456789");
        assert_eq!(tv["labelTextColor"], "#abcdef");
        assert_eq!(tv["loopTextColor"], "#abcdef");
        assert_eq!(tv["scaleLabelColor"], "#abcdef");
        assert_eq!(tv["cScaleLabel11"], "#abcdef");
        assert_eq!(tv["gitBranchLabel7"], "#abcdef");
        assert_eq!(tv["relationLabelColor"], "#abcdef");
        assert_eq!(tv["stateLabelColor"], "#567890");
        assert_eq!(tv["labelBackgroundColor"], "#567890");
        assert_eq!(tv["pieTitleTextColor"], "#678901");
        assert_eq!(tv["pieLegendTextColor"], "#678901");
        assert_eq!(tv["tagLabelBorder"], "#789012");
    }

    #[test]
    fn base_explicit_values_win_after_calculation_including_falsy_values() {
        let tv = resolve(json!({
            "primaryColor": "#181818", "actorBkg": "", "stateBkg": false,
            "taskBkgColor": 0, "pie1": "#123456", "cScale0": "#ffffff",
            "git0": "#ffffff", "gradientStart": "#654321", "useGradient": false,
            "radar": { "axisColor": null }
        }));
        assert_eq!(tv["actorBkg"], "");
        assert_eq!(tv["stateBkg"], false);
        assert_eq!(tv["taskBkgColor"], 0);
        assert_eq!(tv["pie1"], "#123456");
        assert_eq!(tv["labelBoxBkgColor"], "#181818");
        assert_eq!(tv["labelBackgroundColor"], "#181818");
        assert_eq!(tv["stateLabelColor"], "#333");
        assert_eq!(tv["cScale0"], "#ffffff");
        assert_eq!(tv["cScaleInv0"], "rgb(63.75, 63.75, 63.75)");
        assert_eq!(tv["git0"], "#ffffff");
        assert_eq!(tv["gitInv0"], "rgb(63.75, 63.75, 63.75)");
        assert_eq!(tv["gradientStart"], "#654321");
        assert_eq!(tv["useGradient"], false);
        assert_eq!(tv["radar"]["axisColor"], Value::Null);
        assert_eq!(tv["radar"]["graticuleColor"], "#DEDEDE");
        assert_eq!(tv["radar"]["axisStrokeWidth"], 2);
    }

    #[test]
    fn base_validates_colors_only_when_a_transform_uses_them() {
        let tv = resolve(json!({ "secondaryColor": "", "cScale0": "", "git0": "" }));
        assert_eq!(tv["secondaryColor"], "");
        assert_eq!(tv["cScale0"], "");
        assert_eq!(tv["git0"], "");
        assert_eq!(
            tv["activationBkgColor"],
            "hsl(-79.4117647059, 100%, 93.3333333333%)"
        );
        for key in [
            "mainBkg",
            "noteBkgColor",
            "lineColor",
            "primaryTextColor",
            "git7",
            "cScale11",
        ] {
            let mut config = MermaidConfig::from_value(json!({
                "theme": "base", "themeVariables": { (key): "not-a-color" }
            }));
            assert!(apply_theme_defaults(&mut config).is_err(), "{key}");
        }
    }

    #[test]
    fn base_falsy_intermediates_do_not_claim_default_dependents_after_replay() {
        let mut config = MermaidConfig::from_value(json!({"theme": "base"}));
        config.deep_merge_explicit(&json!({"themeVariables": {"actorBkg": "", "stateBkg": false}}));
        apply_theme_defaults(&mut config).unwrap();
        assert_eq!(config.get_str("themeVariables.actorBkg"), Some(""));
        assert_eq!(
            config.get_str("themeVariables.labelBoxBkgColor"),
            Some("#fff4dd")
        );
        assert_eq!(
            config.get_str("themeVariables.stateLabelColor"),
            Some("#333")
        );
        for key in [
            "labelBoxBkgColor",
            "stateLabelColor",
            "labelBackgroundColor",
        ] {
            assert!(
                !config.config_path_overrides_typed_default(&format!("themeVariables.{key}")),
                "{key}"
            );
        }
    }

    #[test]
    fn base_owned_fallbacks_and_nested_fields_keep_their_actual_source() {
        let mut config = MermaidConfig::from_value(json!({"theme": "base"}));
        config.deep_merge_explicit(&json!({"themeVariables": {
            "primaryColor": "#181818", "actorBkg": "", "stateBkg": false,
            "lineColor": "#a0a0a0", "background": "#112233"
        }}));
        apply_theme_defaults(&mut config).unwrap();
        for key in [
            "mainBkg",
            "nodeBkg",
            "personBkg",
            "labelBoxBkgColor",
            "labelBackgroundColor",
            "pie1",
            "requirementBackground",
            "tagLabelBackground",
            "arrowheadColor",
            "defaultLinkColor",
            "relationColor",
            "emArrowhead",
            "emRelationStroke",
            "radar.axisColor",
            "xyChart.backgroundColor",
        ] {
            assert!(
                config.config_path_overrides_typed_default(&format!("themeVariables.{key}")),
                "{key}"
            );
        }
        for key in [
            "stateLabelColor",
            "radar.axisStrokeWidth",
            "xyChart.titleColor",
        ] {
            assert!(
                !config.config_path_overrides_typed_default(&format!("themeVariables.{key}")),
                "{key}"
            );
        }
    }

    #[test]
    fn base_color_limit_failure_keeps_config_transactional() {
        let mut config = MermaidConfig::from_value(json!({"theme": "base"}));
        config.deep_merge_explicit(&json!({"themeVariables": {"THEME_COLOR_LIMIT": 65}}));
        let before = config.as_value().clone();
        assert!(matches!(
            apply_theme_defaults(&mut config),
            Err(ThemeResolutionError::EvaluationLimit(_))
        ));
        assert_eq!(config.as_value(), &before);
    }
}
