use merman_theme_contract::v2::{Appearance, BuiltinPreset, ThemeCompiler, ThemeNotice};
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
        for appearance in preset.appearances().unwrap() {
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
fn equivalent_documents_produce_the_same_fingerprint() {
    let compiler = ThemeCompiler::new();
    let first = compiler
        .compile_json(
            br##"{"schema_version":2,"extends":"studio","seed":{"accent":"#0052CC"}}"##,
            None,
        )
        .unwrap();
    let second = compiler
        .compile_json(
            br##"{"seed":{"accent":"#0052cc"},"extends":"studio","schema_version":2}"##,
            None,
        )
        .unwrap();
    let dark = compiler
        .compile_json(
            br##"{"seed":{"accent":"#0052CC"},"extends":"studio","schema_version":2}"##,
            Some(Appearance::Dark),
        )
        .unwrap();
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_ne!(first.fingerprint(), dark.fingerprint());
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
