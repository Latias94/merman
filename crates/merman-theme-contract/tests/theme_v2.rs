use merman_theme_contract::v2::{
    Appearance, BuiltinPreset, Color, Paint, ResolvedThemeV2, ThemeCompiler, ThemeNotice,
};
use serde_json::{Value, json};

#[test]
fn builtin_presets_match_reference_colors_in_every_appearance() {
    let reference: Value =
        serde_json::from_str(include_str!("fixtures/preset-colors-v2.json")).unwrap();
    let reference = reference.as_object().unwrap();
    let compiler = ThemeCompiler::new().with_strict_contrast(true);
    let mut appearances = 0;

    assert_eq!(BuiltinPreset::ALL.len(), 13);
    for preset in BuiltinPreset::ALL {
        let document = preset.document().unwrap();
        assert_eq!(
            preset.supported_appearances(),
            document.appearances().collect::<Vec<_>>()
        );
        let implicit = compiler.compile(document, None).unwrap();
        assert_eq!(preset.default_appearance(), implicit.appearance());
        assert!(std::sync::Arc::ptr_eq(
            &compiler.compile_preset(*preset, None).unwrap(),
            &compiler
                .compile_preset(*preset, Some(preset.default_appearance()))
                .unwrap(),
        ));
        for &appearance in preset.supported_appearances() {
            appearances += 1;
            let key = format!("{preset}:{appearance}");
            let actual = compiler.compile_preset(*preset, Some(appearance)).unwrap();
            let expected = &reference[&key];
            assert_eq!(json!(actual.roles()), expected["roles"], "{key} roles");
            assert_eq!(json!(actual.data().ink), expected["ink"], "{key} ink");
            assert_eq!(json!(actual.data().fill), expected["fill"], "{key} fill");
            assert_eq!(
                json!(actual.data().fill_ink),
                expected["fill_ink"],
                "{key} fill ink"
            );
            assert_eq!(
                json!(actual.family("pie").data.fill_ink),
                expected["pie_fill_ink"],
                "{key} pie fill ink"
            );
        }
    }
    assert_eq!(appearances, 19);
    assert_eq!(appearances, reference.len());
}

#[test]
fn extends_merges_fields_and_applies_shared_seed_to_each_appearance() {
    let compiler = ThemeCompiler::new();
    let patch = br##"{
        "schema_version": 2,
        "extends": "studio",
        "seed": { "accent": "#B42318" },
        "typography": { "base_size_px": 16 },
        "form": { "chart": { "line_markers": false } },
        "appearances": { "dark": { "seed": { "surface": "#18202F" } } }
    }"##;
    for appearance in [Appearance::Light, Appearance::Dark] {
        let base = compiler
            .compile_preset(BuiltinPreset::Studio, Some(appearance))
            .unwrap();
        let resolved = compiler.compile_json(patch, Some(appearance)).unwrap();
        assert_eq!(resolved.appearance(), appearance);
        assert_eq!(json!(resolved.roles().status_info), "#B42318");
        assert_eq!(resolved.roles().canvas, base.roles().canvas);
        assert_eq!(resolved.typography().mono, base.typography().mono);
        assert_eq!(resolved.typography().base_size_px, 16.0);
        assert_eq!(resolved.form().radius, base.form().radius);
        assert!(resolved.form().chart.gridlines);
        assert!(!resolved.form().chart.line_markers);
        assert_eq!(resolved.data().ink, base.data().ink);
    }
    let dark = compiler
        .compile_json(patch, Some(Appearance::Dark))
        .unwrap();
    assert_eq!(json!(dark.roles().surface_node), "#18202F");
}

#[test]
fn json_errors_keep_paths_and_source_locations() {
    let compiler = ThemeCompiler::new();
    let duplicate =
        b"{\n  \"schema_version\": 2,\n  \"form\": {\"radius\": 6, \"\\u0072adius\": 8}\n}";
    let error = compiler.compile_json(duplicate, None).unwrap_err();
    assert!(error.to_string().contains("duplicate"));
    assert!(error.line().is_some_and(|line| line == 3));
    assert!(error.path().is_some_and(|path| path.contains("form")));

    let invalid = b"{\n  \"schema_version\": 2,\n  \"extends\": \"studio\",\n  \"seed\": { \"accent\": \"#ABC\" }\n}";
    let error = compiler.compile_json(invalid, None).unwrap_err();
    assert_eq!(error.line(), Some(4));
    assert!(error.column().is_some_and(|column| column > 0));
    assert_eq!(error.path(), Some("seed.accent"));
}

#[test]
fn rejects_invalid_documents_and_unavailable_appearances() {
    let compiler = ThemeCompiler::new();
    for document in [
        json!({"schema_version": 1, "extends": "studio"}),
        json!({"schema_version": 2}),
        json!({"schema_version": 2, "id": "self", "extends": "self"}),
        json!({"schema_version": 2, "extends": "blueprint", "default_appearance": "light"}),
        json!({"schema_version": 2, "appearances": {"light": {"seed": {"canvas": "#FFFFFF", "ink": "#111111"}}}}),
        json!({"schema_version": 2, "extends": "studio", "mermaid": {}}),
        json!({"schema_version": 2, "extends": "studio", "form": {"radius": 25}}),
        json!({"schema_version": 2, "extends": "studio", "form": {"radius": null}}),
        json!({"schema_version": 2, "extends": "studio", "form": {"elevation": {"kind": "glow", "color": "#00FFFF", "blur": 20}}}),
    ] {
        assert!(
            compiler
                .compile_json(document.to_string().as_bytes(), None)
                .is_err(),
            "accepted {document}"
        );
    }
    assert!(
        compiler
            .compile_preset(BuiltinPreset::Blueprint, Some(Appearance::Light))
            .is_err()
    );
    assert!(
        compiler
            .with_max_bytes(10)
            .compile_json(br#"{"schema_version":2,"extends":"studio"}"#, None)
            .is_err()
    );
}

#[test]
fn a_custom_theme_reports_low_contrast_and_can_adjust_it() {
    let input = br##"{
      "schema_version": 2,
      "extends": "studio",
      "roles": { "text.muted": "#EEEEEE" }
    }"##;
    let compiler = ThemeCompiler::new();
    let original = compiler.compile_json(input, None).unwrap();
    assert!(original.notices().iter().any(|notice| matches!(notice,
        ThemeNotice::LowContrast { foreground, .. } if foreground == "text.muted"
    )));
    assert!(
        compiler
            .clone()
            .with_strict_contrast(true)
            .compile_json(input, None)
            .is_err()
    );
    let fixed = compiler
        .with_strict_contrast(true)
        .with_auto_fix_contrast(true)
        .compile_json(input, None)
        .unwrap();
    assert!(fixed.notices().iter().any(|notice| matches!(notice,
        ThemeNotice::ContrastAdjusted { foreground, .. } if foreground == "text.muted"
    )));
    assert_ne!(original.roles().text_muted, fixed.roles().text_muted);
}

#[test]
fn family_contrast_reports_inherited_notices_once() {
    let shared = json!({
        "schema_version": 2,
        "extends": "studio",
        "roles": {"text.muted": "#EEEEEE"}
    });
    let mut overridden = shared.clone();
    overridden["overrides"] = json!([
        {"families": ["flowchart"], "form": {"radius": 12}},
        {"families": ["state"], "typography": {"base_size_px": 18}}
    ]);
    for auto_fix in [false, true] {
        let compiler = ThemeCompiler::new().with_auto_fix_contrast(auto_fix);
        let shared = compiler
            .compile_json(shared.to_string().as_bytes(), None)
            .unwrap();
        let overridden = compiler
            .compile_json(overridden.to_string().as_bytes(), None)
            .unwrap();
        assert_eq!(shared.notices().len(), if auto_fix { 1 } else { 2 });
        assert_eq!(overridden.notices(), shared.notices());
        for family in ["flowchart", "state"] {
            assert_eq!(overridden.family(family).roles, *shared.roles());
            assert_eq!(overridden.family(family).data, *shared.data());
        }
        assert_eq!(overridden.family("flowchart").form.radius, 12.0);
        assert_eq!(overridden.family("state").typography.base_size_px, 18.0);
    }
}

#[test]
fn family_contrast_recomputes_adjustments_and_keeps_authored_titles() {
    let document = br##"{
        "schema_version": 2,
        "extends": "studio",
        "roles": {
            "canvas": "#FFFFFF", "surface.node": "#FFFFFF", "surface.container": "#FFFFFF",
            "text.primary": "#EEEEEE", "text.title": "#CCCCCC"
        },
        "overrides": [
            {"families": ["flowchart"], "roles": {
                "canvas": "#000000", "surface.node": "#000000", "surface.container": "#000000"
            }},
            {"families": ["class"], "roles": {
                "surface.callout": "#713F12", "text.title": "#112233"
            }}
        ]
    }"##;
    let resolved = ThemeCompiler::new()
        .with_auto_fix_contrast(true)
        .compile_json(document, None)
        .unwrap();
    let flowchart = &resolved.family("flowchart").roles;
    assert_eq!(flowchart.text_primary, Color::rgb(238, 238, 238));
    assert_eq!(flowchart.text_title, Color::rgb(204, 204, 204));
    assert_ne!(flowchart.text_primary, resolved.roles().text_primary);
    assert_ne!(flowchart.text_title, resolved.roles().text_title);

    let class = &resolved.family("class").roles;
    assert_eq!(class.text_title, Color::rgb(17, 34, 51));
    assert_eq!(class.text_primary, resolved.roles().text_primary);
    assert_ne!(class.text_callout, resolved.roles().text_callout);
    assert!(resolved.notices().iter().any(|notice| matches!(notice,
        ThemeNotice::ContrastAdjusted { foreground, color }
            if foreground == "family_overrides[\"class\"].text.callout"
                && *color == class.text_callout
    )));
    assert!(!resolved.notices().iter().any(|notice| matches!(notice,
        ThemeNotice::ContrastAdjusted { foreground, .. }
            if foreground == "family_overrides[\"class\"].text.primary"
    )));
}

#[test]
fn family_contrast_reports_changed_inputs_even_at_the_same_ratio() {
    let mut document = json!({
        "schema_version": 2,
        "extends": "studio",
        "roles": {"text.callout": "#00000000"},
        "overrides": [{"families": ["sequence"], "roles": {"surface.callout": "#FFFFFF"}}]
    });
    let compiler = ThemeCompiler::new();
    let resolved = compiler
        .compile_json(document.to_string().as_bytes(), None)
        .unwrap();
    let inherited_fail = json!({
        "kind": "low_contrast", "foreground": "text.callout", "background": "surface.callout",
        "ratio": 1.0, "minimum": 4.5
    });
    let mut family_fail = inherited_fail.clone();
    family_fail["foreground"] = json!("family_overrides[\"sequence\"].text.callout");
    assert_eq!(
        json!(resolved.notices()),
        json!([inherited_fail, family_fail])
    );

    let strict = compiler.with_strict_contrast(true);
    let error = strict
        .compile_json(document.to_string().as_bytes(), None)
        .unwrap_err();
    assert_eq!(error.path(), Some("text.callout"));
    document.as_object_mut().unwrap().remove("roles");
    document["overrides"][0]["roles"]["text.callout"] = json!("#FFFFFF");
    let error = strict
        .compile_json(document.to_string().as_bytes(), None)
        .unwrap_err();
    assert_eq!(
        error.path(),
        Some("family_overrides[\"sequence\"].text.callout")
    );
    assert_eq!((error.line(), error.column()), (None, None));
}

#[test]
fn family_contrast_uses_pie_slice_opacity_even_without_a_stored_override() {
    let mut document = json!({
        "schema_version": 2,
        "extends": "studio",
        "form": {"chart": {"pie_opacity": 0.65}},
        "appearances": {"light": {"data": {
            "fill": ["#444444", "#444444", "#444444", "#444444"], "fill_ink": "#FFFFFF"
        }}}
    });
    let compiler = ThemeCompiler::new();
    let foreground = "family_overrides[\"pie\"].data.fill_ink[0]";
    for explicit in [false, true] {
        if explicit {
            document["form"]["chart"]["pie_opacity"] = json!(1.0);
            document["overrides"] = json!([
                {"families": ["pie"], "form": {"chart": {"pie_opacity": 0.65}}}
            ]);
        }
        let text = document.to_string();
        let resolved = compiler.compile_json(text.as_bytes(), None).unwrap();
        assert!(low_contrast(&resolved, foreground, "pie.data.fill") < 4.5);
        assert_eq!(
            resolved.family("pie").data.fill_ink,
            resolved.data().fill_ink
        );
        assert_eq!(json!(resolved).get("family_overrides").is_some(), explicit);
        let strict = compiler.clone().with_strict_contrast(true);
        assert_eq!(
            strict
                .compile_json(text.as_bytes(), None)
                .unwrap_err()
                .path(),
            Some(foreground)
        );
        let fixed = strict
            .with_auto_fix_contrast(true)
            .compile_json(text.as_bytes(), None)
            .unwrap();
        assert_eq!(fixed.data().fill_ink(0), Color::rgb(255, 255, 255));
        assert_ne!(
            fixed.family("pie").data.fill_ink(0),
            fixed.data().fill_ink(0)
        );
        assert!(fixed.notices().iter().any(|notice| matches!(notice,
            ThemeNotice::ContrastAdjusted { foreground: name, color }
                if name == foreground && *color == fixed.family("pie").data.fill_ink(0)
        )));
    }

    document["overrides"][0]["form"]["chart"]["pie_opacity"] = json!(1.0);
    document["appearances"]["light"]["data"]["fill"][0] = json!("#FFFFFF");
    for auto_fix in [false, true] {
        let resolved = compiler
            .clone()
            .with_auto_fix_contrast(auto_fix)
            .compile_json(document.to_string().as_bytes(), None)
            .unwrap();
        assert_eq!(resolved.notices().len(), 1);
        assert_eq!(
            json!(resolved.notices())[0]["foreground"],
            "data.fill_ink[0]"
        );
        assert_eq!(
            resolved.family("pie").data.fill_ink,
            resolved.data().fill_ink
        );
    }
}

#[test]
fn contrast_checks_between_canvas_stops_and_through_translucent_surfaces() {
    let input = br##"{
        "schema_version": 2,
        "extends": "studio",
        "roles": {
            "canvas": {"kind": "linear_gradient", "angle_deg": 90, "stops": [
                {"offset": 0, "color": "#FF0000"}, {"offset": 1, "color": "#008A00"}
            ]},
            "surface.node": "#00000040",
            "text.primary": "#000000",
            "text.muted": "#000000",
            "text.title": "#000000"
        },
        "form": {"chart": {"pie_opacity": 0.8}},
        "appearances": {"light": {"data": {
            "ink": ["#000000", "#000000", "#000000", "#000000"],
            "fill": ["#00000050", "#FFFFFF", "#FFFFFF", "#FFFFFF"],
            "fill_ink": "#000000"
        }}}
    }"##;
    let compiler = ThemeCompiler::new();
    let theme = compiler.compile_json(input, None).unwrap();
    let ratio = low_contrast(&theme, "text.primary", "canvas");
    assert!((ratio - 2.75317).abs() < 0.001, "{ratio}");
    let shaded_ratio = low_contrast(&theme, "text.primary", "surface.node");
    assert!((shaded_ratio - 1.97468).abs() < 0.001, "{shaded_ratio}");
    assert_eq!(
        low_contrast(
            &theme,
            "family_overrides[\"pie\"].data.fill_ink[0]",
            "pie.data.fill"
        ),
        shaded_ratio
    );

    let error = compiler
        .with_strict_contrast(true)
        .compile_json(input, None)
        .unwrap_err();
    assert_eq!(error.path(), Some("text.primary"));
    assert_eq!((error.line(), error.column()), (None, None));
    assert_eq!(
        error.to_string(),
        "text.primary on canvas has 2.75:1 contrast, below 4.5:1"
    );
}

#[test]
fn translucent_pie_slices_and_shared_fills_use_their_own_contrasting_ink() {
    let input = json!({
        "schema_version": 2,
        "extends": "studio",
        "form": {"chart": {"pie_opacity": 0.65}},
        "appearances": {"light": {"data": {
            "fill": ["#444444", "#444444", "#444444", "#444444"],
            "fill_ink": "auto"
        }}},
        "overrides": [{"families": ["mindmap"], "form": {"chart": {"pie_opacity": 0.8}}}]
    });
    let theme = ThemeCompiler::new()
        .with_strict_contrast(true)
        .compile_json(input.to_string().as_bytes(), None)
        .unwrap();
    let pie = theme.family("pie");
    let mindmap = theme.family("mindmap");
    let pie_background = pie
        .data
        .fill(0)
        .composite(pie.roles.canvas.first_color(), 0.65);
    assert!(pie.data.fill_ink(0).contrast(pie_background) >= 4.5);
    assert!(mindmap.data.fill_ink(0).contrast(mindmap.data.fill(0)) >= 4.5);
    assert_eq!(theme.data().fill_ink(0), mindmap.data.fill_ink(0));
    assert_ne!(pie.data.fill_ink(0), mindmap.data.fill_ink(0));
}

#[test]
fn automatic_pie_ink_uses_the_entire_canvas_gradient() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
              "schema_version": 2,
              "extends": "studio",
              "form": {"chart": {"pie_opacity": 0.65}},
              "roles": {"canvas": {"kind": "linear_gradient", "stops": [
                {"offset": 0, "color": "#FFFFFF"}, {"offset": 1, "color": "#000000"}
              ]}},
              "appearances": {"light": {"data": {
                "fill": ["#2C2C2C", "#2C2C2C", "#2C2C2C", "#2C2C2C"],
                "fill_ink": "auto"
              }}}
            }"##,
            None,
        )
        .unwrap();
    let pie = theme.family("pie");
    assert_eq!(pie.data.fill_ink(0), Color::rgb(255, 255, 255));
    assert!(
        Paint::Solid(pie.data.fill_ink(0)).minimum_contrast_over(
            &Paint::Solid(pie.data.fill(0)),
            pie.form.chart.pie_opacity,
            Some(&pie.roles.canvas),
            theme.appearance(),
        ) >= 4.5
    );
    assert!(!theme.notices().iter().any(|notice| matches!(
        notice,
        ThemeNotice::LowContrast {
            background: "pie.data.fill",
            ..
        }
    )));
}

#[test]
fn a_transparent_seed_accent_falls_back_to_readable_title_ink() {
    let theme = ThemeCompiler::new()
        .with_strict_contrast(true)
        .compile_json(
            br##"{"schema_version":2,"extends":"studio","seed":{"accent":"#00000000"}}"##,
            None,
        )
        .unwrap();
    assert_eq!(theme.roles().text_title, theme.roles().text_primary);
}

#[test]
fn derived_title_uses_the_resolved_canvas_and_keeps_authored_titles() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,
                "appearances":{"light":{"seed":{"canvas":"#00000000","accent":"#000000","ink":"#FFFFFF"}}},
                "roles":{"canvas":"#000000"},
                "overrides":[
                    {"families":["flowchart"],"roles":{"canvas":"#FFFFFF"}},
                    {"families":["pie"],"roles":{"text.title":"#FF8800"}}
                ]
            }"##,
            None,
        )
        .unwrap();
    assert_eq!(theme.roles().text_title, Color::rgb(255, 255, 255));
    assert_eq!(
        theme.family("flowchart").roles.text_title,
        Color::rgb(0, 0, 0)
    );
    assert_eq!(
        theme.family("pie").roles.text_title,
        Color::rgb(255, 136, 0)
    );
}

#[test]
fn contrast_accounts_for_transparent_stop_color_before_compositing() {
    let input = br##"{
        "schema_version": 2,
        "extends": "studio",
        "roles": {
            "canvas": "#818181",
            "surface.node": {"kind": "linear_gradient", "stops": [
                {"offset": 0.2, "color": "#00000000"},
                {"offset": 0.8, "color": "#818181CC", "opacity": 0.9}
            ]},
            "text.primary": "#000000"
        }
    }"##;
    let theme = ThemeCompiler::new().compile_json(input, None).unwrap();
    let painted = theme.roles().surface_node.first_color();
    assert_eq!(painted.channels()[3], 0);
    // Both stops composite to #818181. Halfway, the lighter stop's opacity and RGB are both halved.
    let ratio = low_contrast(&theme, "text.primary", "surface.node");
    assert!((ratio - 3.869905).abs() < 0.001, "{ratio}");
}

#[test]
fn a_solid_and_a_continuous_gradient_can_cross_at_equal_luminance() {
    let solid = Paint::Solid(Color::rgb(117, 117, 117));
    let gradient = |first, last| {
        serde_json::from_value::<Paint>(json!({"kind": "linear_gradient", "stops": [
            {"offset": first, "color": "#000000"}, {"offset": last, "color": "#FFFFFF"}
        ]}))
        .unwrap()
    };
    let continuous = gradient(0.0, 1.0);
    assert_eq!(
        solid.minimum_contrast_over(&continuous, 1.0, None, Appearance::Light),
        1.0
    );
    assert_eq!(
        continuous.minimum_contrast_over(&solid, 1.0, None, Appearance::Light),
        1.0
    );

    let hard_stop = gradient(0.5, 0.5);
    let ratio = solid.minimum_contrast_over(&hard_stop, 1.0, None, Appearance::Light);
    assert!((ratio - 4.557768).abs() < 0.001, "{ratio}");
}

fn low_contrast(theme: &ResolvedThemeV2, foreground: &str, background: &str) -> f64 {
    theme
        .notices()
        .iter()
        .find_map(|notice| match notice {
            ThemeNotice::LowContrast {
                foreground: fg,
                background: bg,
                ratio,
                ..
            } if fg == foreground && *bg == background => Some(*ratio),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing contrast notice for {foreground} on {background}"))
}

#[test]
fn family_overrides_apply_in_order_to_the_selected_appearance() {
    let input = br##"{
      "schema_version": 2,
      "extends": "studio",
      "overrides": [
        {"families": ["flowchart", "state"], "form": {"radius": 10}},
        {"families": ["flowchart"], "appearance": "dark", "form": {"radius": 14}}
      ]
    }"##;
    let compiler = ThemeCompiler::new();
    let light = compiler
        .compile_json(input, Some(Appearance::Light))
        .unwrap();
    let dark = compiler
        .compile_json(input, Some(Appearance::Dark))
        .unwrap();
    assert_eq!(light.family("flowchart").form.radius, 10.0);
    assert_eq!(dark.family("flowchart").form.radius, 14.0);
    assert_eq!(dark.family("state").form.radius, 10.0);
    assert_eq!(dark.family("sequence").form.radius, dark.form().radius);
}
