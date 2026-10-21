use merman_core::Engine;
use merman_render::presentation::{
    HostTheme, HostThemePreset, Presentation, PresentationProfile, ThemeRole,
    presentation_profile_descriptors, theme_preset_descriptors,
};

fn effective_config(presentation: Presentation, source: &str) -> serde_json::Value {
    presentation
        .resolve()
        .materialize_engine(Engine::new())
        .parse_metadata_sync(source)
        .expect("metadata parse should succeed")
        .effective_config
        .as_value()
        .clone()
}

#[test]
fn source_palette_replaces_host_colors_without_losing_host_typography() {
    for source in [
        "---\nconfig:\n  theme: base\n  themeVariables:\n    quadrant1Fill: '#e8f5e9'\n---\nquadrantChart\n  Alpha: [0.7, 0.8]",
        "%%{init: {'theme': 'base', 'themeVariables': {'quadrant1Fill': '#e8f5e9'}}}%%\nquadrantChart\n  Alpha: [0.7, 0.8]",
    ] {
        let actual = effective_config(
            Presentation::new().with_theme(HostTheme::from_preset(HostThemePreset::EditorDark)),
            source,
        );
        assert_eq!(actual["themeVariables"]["quadrant1Fill"], "#e8f5e9");
        assert_eq!(actual["themeVariables"]["quadrantPointTextFill"], "#333");
        assert!(
            actual["themeVariables"]["fontFamily"]
                .as_str()
                .unwrap()
                .contains("Segoe UI")
        );
        assert_eq!(actual["themeVariables"]["fontSize"], "14px");
    }
}

#[test]
fn site_palette_replaces_host_colors_and_retains_raw_initialization_variables() {
    let raw = merman_core::MermaidConfig::from_value(serde_json::json!({
        "theme": "forest",
        "themeVariables": {"primaryColor": "#fedcba", "rowOdd": "#123456"}
    }));
    let source = "erDiagram\n  USER ||--o{ ORDER : places";
    let baseline = Engine::new()
        .with_site_config(raw.clone())
        .parse_metadata_sync(source)
        .unwrap();
    let actual = Presentation::new()
        .with_theme(HostTheme::from_preset(HostThemePreset::EditorDark))
        .resolve()
        .materialize_engine(Engine::new().with_site_config(raw))
        .parse_metadata_sync(source)
        .unwrap();
    for key in [
        "primaryColor",
        "rowOdd",
        "rowEven",
        "relationColor",
        "textColor",
    ] {
        assert_eq!(
            actual.effective_config.as_value()["themeVariables"][key],
            baseline.effective_config.as_value()["themeVariables"][key],
            "{key}"
        );
    }
}

#[test]
fn timeline_source_palette_rederives_scale_labels_from_raw_site_variables() {
    let raw = merman_core::MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {"primaryColor": "#abcdef", "labelTextColor": "#172b4d"}
    }));
    let source = "---\nconfig:\n  theme: base\n  themeVariables:\n    primaryTextColor: '#123456'\n---\ntimeline\n  section Release\n    Alpha : Ship";
    let baseline = Engine::new()
        .with_site_config(raw.clone())
        .parse_metadata_sync(source)
        .unwrap();
    let actual = Presentation::new()
        .with_theme(HostTheme::from_preset(HostThemePreset::EditorDark))
        .resolve()
        .materialize_engine(Engine::new().with_site_config(raw))
        .parse_metadata_sync(source)
        .unwrap();
    for key in [
        "primaryColor",
        "primaryTextColor",
        "cScale0",
        "cScaleLabel0",
        "cScaleLabel1",
    ] {
        assert_eq!(
            actual.effective_config.as_value()["themeVariables"][key],
            baseline.effective_config.as_value()["themeVariables"][key],
            "{key}"
        );
    }
}

#[test]
fn presentation_installation_preserves_raw_site_layers_and_replaces_its_own_defaults() {
    let dark = Presentation::new()
        .with_theme(HostTheme::from_preset(HostThemePreset::EditorDark))
        .resolve();
    let light = Presentation::new()
        .with_theme(HostTheme::from_preset(HostThemePreset::EditorLight))
        .resolve();
    let raw = merman_core::MermaidConfig::from_value(serde_json::json!({
        "theme": "base", "themeVariables": {"primaryColor": "#abcdef"}
    }));
    let source = "sequenceDiagram\nAlice->>Bob: Hello";
    let before = dark.materialize_engine(Engine::new().with_site_config(raw.clone()));
    let after = dark.materialize_engine(Engine::new()).with_site_config(raw);
    let parse = |engine: Engine| engine.parse_metadata_sync(source).unwrap().effective_config;
    assert_eq!(parse(before), parse(after));
    assert_eq!(
        parse(light.materialize_engine(dark.materialize_engine(Engine::new()))),
        parse(light.materialize_engine(Engine::new()))
    );
    assert_eq!(
        parse(
            dark.materialize_engine(Engine::new())
                .with_exact_site_config(None)
        ),
        parse(Engine::new())
    );
}

#[test]
fn same_value_source_theme_is_authored_and_missing_palette_roles_keep_host_defaults() {
    let presentation =
        Presentation::new().with_theme(HostTheme::from_preset(HostThemePreset::EditorDark));
    let actual = effective_config(
        presentation,
        "---\nconfig:\n  theme: base\n---\nsequenceDiagram\nAlice->>Bob: Hello",
    );
    assert_eq!(actual["themeVariables"]["primaryTextColor"], "#333");
    // This extension role has no upstream palette value, so the preset remains its fallback.
    assert_eq!(actual["themeVariables"]["faceColor"], "#111827");
}

#[test]
fn selected_palette_preserves_optional_terminal_alias_fallbacks() {
    let source = "---\nconfig:\n  theme: forest\n---\nerDiagram\nA ||--o{ B : owns";
    let baseline = Engine::new().parse_metadata_sync(source).unwrap();
    let baseline = &baseline.effective_config.as_value()["themeVariables"];
    for (preset, surface) in [
        (HostThemePreset::EditorLight, "#f8fafc"),
        (HostThemePreset::EditorDark, "#111827"),
    ] {
        let actual = effective_config(
            Presentation::new().with_theme(HostTheme::from_preset(preset)),
            source,
        );
        let actual = &actual["themeVariables"];
        for (optional, fallback) in [
            ("nodeTextColor", "textColor"),
            ("stateBorder", "nodeBorder"),
            ("requirementEdgeLabelBackground", "edgeLabelBackground"),
            ("commitLineColor", "lineColor"),
        ] {
            assert!(baseline.get(optional).is_none(), "{optional}");
            assert!(
                actual.get(optional).is_none(),
                "{preset:?}: {optional} must retain its selected-palette reader fallback"
            );
            assert_eq!(
                actual[fallback], baseline[fallback],
                "{preset:?}/{fallback}"
            );
        }
        assert_eq!(actual["faceColor"], surface);
    }
}

#[test]
fn explicit_terminal_aliases_outrank_selected_palette_fallbacks() {
    let variables = serde_json::json!({
        "nodeTextColor": "#123456",
        "stateBorder": "#234567",
        "requirementEdgeLabelBackground": "#345678",
        "commitLineColor": "#456789"
    });
    for from_source in [false, true] {
        let config = serde_json::json!({"theme": "forest", "themeVariables": variables});
        let (source, engine) = if from_source {
            (
                format!("---\nconfig: {config}\n---\nerDiagram\nA ||--o{{ B : owns"),
                Engine::new(),
            )
        } else {
            (
                "erDiagram\nA ||--o{ B : owns".to_owned(),
                Engine::new().with_site_config(merman_core::MermaidConfig::from_value(config)),
            )
        };
        let parsed = Presentation::new()
            .with_theme(HostTheme::from_preset(HostThemePreset::EditorDark))
            .resolve()
            .materialize_engine(engine)
            .parse_metadata_sync(&source)
            .unwrap();
        let actual = &parsed.effective_config.as_value()["themeVariables"];
        for (key, value) in variables.as_object().unwrap() {
            assert_eq!(&actual[key], value, "{key}/source={from_source}");
        }
        assert_eq!(actual["faceColor"], "#111827");
    }
}

#[test]
fn filtered_source_theme_and_null_sentinel_preserve_initialized_host_palette() {
    let resolved = Presentation::new()
        .with_theme(HostTheme::from_preset(HostThemePreset::EditorDark))
        .resolve();
    let source = "---\nconfig:\n  theme: base\n  themeVariables:\n    primaryColor: '#abcdef'\n---\nsequenceDiagram\nAlice->>Bob: Hello";
    let actual =
        resolved
            .materialize_engine(Engine::new().with_site_config(
                merman_core::MermaidConfig::from_value(serde_json::json!({"secure": ["theme"]})),
            ))
            .parse_metadata_sync(source)
            .unwrap();
    assert_eq!(
        actual
            .effective_config
            .get_str("themeVariables.primaryTextColor"),
        Some("#e5e7eb")
    );
    let sentinel = "---\nconfig:\n  theme: 'null'\n  themeVariables:\n    primaryColor: '#abcdef'\n---\nsequenceDiagram\nAlice->>Bob: Hello";
    let actual = resolved
        .materialize_engine(Engine::new())
        .parse_metadata_sync(sentinel)
        .unwrap();
    assert_eq!(
        actual
            .effective_config
            .get_str("themeVariables.primaryTextColor"),
        Some("#e5e7eb")
    );
    assert_eq!(
        actual
            .effective_config
            .get_str("themeVariables.primaryColor"),
        Some("#abcdef")
    );
}

#[test]
fn detector_unrelated_root_write_does_not_restore_host_palette() {
    fn detect(_source: &str, config: &mut merman_core::MermaidConfig) -> bool {
        config.as_value_mut()["detectorSelected"] = serde_json::json!(true);
        true
    }
    let mut engine = Engine::new();
    *engine.registry_mut() = merman_core::DetectorRegistry::new();
    engine.registry_mut().add_fn("sequence", detect);
    let actual = Presentation::new()
        .with_theme(HostTheme::from_preset(HostThemePreset::EditorDark))
        .resolve()
        .materialize_engine(engine)
        .parse_metadata_sync("---\nconfig:\n  theme: base\n---\ncustom")
        .unwrap();
    assert_eq!(
        actual.effective_config.get_bool("detectorSelected"),
        Some(true)
    );
    assert_eq!(
        actual
            .effective_config
            .get_str("themeVariables.primaryTextColor"),
        Some("#333")
    );
    assert_eq!(
        actual
            .effective_config
            .get_str("themeVariables.quadrantPointTextFill"),
        Some("#333")
    );
}

#[test]
fn detector_root_palette_write_preserves_only_changed_nested_values() {
    fn detect(_source: &str, config: &mut merman_core::MermaidConfig) -> bool {
        let root = config.as_value_mut();
        root["themeVariables"]["radar"]["axisColor"] = serde_json::json!("#123456");
        root["themeVariables"]
            .as_object_mut()
            .unwrap()
            .remove("primaryTextColor");
        true
    }
    let mut engine = Engine::new();
    *engine.registry_mut() = merman_core::DetectorRegistry::new();
    engine.registry_mut().add_fn("sequence", detect);
    let actual = Presentation::new()
        .with_theme(HostTheme::from_preset(HostThemePreset::EditorDark))
        .resolve()
        .materialize_engine(engine)
        .parse_metadata_sync("---\nconfig:\n  theme: base\n  themeVariables:\n    primaryTextColor: '#654321'\n---\ncustom")
        .unwrap();
    assert_eq!(
        actual
            .effective_config
            .get_str("themeVariables.radar.axisColor"),
        Some("#123456")
    );
    assert_eq!(
        actual
            .effective_config
            .get_str("themeVariables.primaryTextColor"),
        Some("#333")
    );
    assert_eq!(
        actual.effective_config.get_str("radar.axisColor"),
        Some("#123456")
    );
}

#[test]
fn detector_same_value_paint_writes_remain_authoritative_during_palette_recalculation() {
    fn detect(_source: &str, config: &mut merman_core::MermaidConfig) -> bool {
        config.set_value(
            "themeVariables.primaryTextColor",
            serde_json::json!("#e5e7eb"),
        );
        config.deep_merge(&serde_json::json!({"themeVariables": {"lineColor": "#94a3b8"}}));
        config.as_value_mut()["detectorSelected"] = serde_json::json!(true);
        true
    }
    let mut engine = Engine::new();
    *engine.registry_mut() = merman_core::DetectorRegistry::new();
    engine.registry_mut().add_fn("sequence", detect);
    let actual = Presentation::new()
        .with_theme(HostTheme::from_preset(HostThemePreset::EditorDark))
        .resolve()
        .materialize_engine(engine)
        .parse_metadata_sync("---\nconfig:\n  theme: base\n---\ncustom")
        .unwrap();
    assert_eq!(
        actual
            .effective_config
            .get_str("themeVariables.primaryTextColor"),
        Some("#e5e7eb")
    );
    assert_eq!(
        actual.effective_config.get_str("themeVariables.lineColor"),
        Some("#94a3b8")
    );
    assert_eq!(
        actual
            .effective_config
            .get_str("themeVariables.quadrantPointTextFill"),
        Some("#e5e7eb")
    );
    assert_eq!(
        actual.effective_config.get_bool("detectorSelected"),
        Some(true)
    );
}

#[test]
fn selected_palette_reaches_root_family_terminals_without_erasing_explicit_styles() {
    let actual = effective_config(
        Presentation::new().with_theme(HostTheme::from_preset(HostThemePreset::EditorDark)),
        "---\nconfig:\n  theme: base\n  treemap:\n    labelColor: '#abcdef'\n  themeVariables:\n    textColor: '#112233'\n    radar:\n      axisColor: '#123456'\n---\nsequenceDiagram\nAlice->>Bob: Hello",
    );
    assert_eq!(actual["treemap"]["labelColor"], "#abcdef");
    assert_eq!(actual["treemap"]["valueColor"], "#112233");
    assert_eq!(actual["radar"]["axisColor"], "#123456");
    assert_eq!(actual["packet"]["labelColor"], "#e5e7eb");
    // Base has no darkMode role; absence must preserve the independent host fallback.
    assert_eq!(actual["darkMode"], true);
}

#[test]
fn empty_presentation_preserves_default_effective_config() {
    let source = "sequenceDiagram\nAlice->>Bob: Hello";
    let baseline = Engine::new()
        .parse_metadata_sync(source)
        .expect("baseline metadata should parse")
        .effective_config
        .as_value()
        .clone();

    assert_eq!(effective_config(Presentation::new(), source), baseline);
}

#[test]
fn theme_catalog_contains_only_the_seven_editor_presets() {
    let ids = theme_preset_descriptors()
        .iter()
        .map(|descriptor| descriptor.id())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        [
            "editor-light",
            "editor-dark",
            "one-dark",
            "gruvbox-light",
            "gruvbox-dark",
            "ayu-light",
            "ayu-dark",
        ]
    );
    assert_eq!(HostThemePreset::ALL.len(), 7);
}

#[test]
fn bundled_themes_publish_stable_semantic_representatives() {
    let expected = [
        (
            HostThemePreset::EditorLight,
            false,
            "#ffffff",
            "#64748b",
            "#2563eb",
        ),
        (
            HostThemePreset::EditorDark,
            true,
            "#0f172a",
            "#94a3b8",
            "#60a5fa",
        ),
        (
            HostThemePreset::OneDark,
            true,
            "#282c34",
            "#61afef",
            "#61afef",
        ),
        (
            HostThemePreset::GruvboxLight,
            false,
            "#fbf1c7",
            "#7c6f64",
            "#458588",
        ),
        (
            HostThemePreset::GruvboxDark,
            true,
            "#282828",
            "#d5c4a1",
            "#83a598",
        ),
        (
            HostThemePreset::AyuLight,
            false,
            "#fcfcfc",
            "#5c6166",
            "#55b4d4",
        ),
        (
            HostThemePreset::AyuDark,
            true,
            "#0b0e14",
            "#59c2ff",
            "#59c2ff",
        ),
    ];

    for (preset, dark, canvas, line, first_series) in expected {
        let theme = HostTheme::from_preset(preset);
        assert_eq!(
            theme.appearance().is_some_and(|value| value.is_dark()),
            dark
        );
        assert_eq!(theme.role(ThemeRole::Canvas), Some(canvas));
        assert_eq!(theme.role(ThemeRole::Line), Some(line));
        assert_eq!(
            theme.series_palette().first().map(String::as_str),
            Some(first_series)
        );
        assert!(
            theme
                .font_family()
                .is_some_and(|value| value.contains("Segoe UI"))
        );
        assert_eq!(theme.font_size(), Some("14px"));
    }
}

#[test]
fn bundled_theme_text_and_lines_remain_readable_on_the_canvas() {
    for preset in HostThemePreset::ALL {
        let theme = HostTheme::from_preset(preset);
        let canvas = theme.role(ThemeRole::Canvas).unwrap();
        let text = theme.role(ThemeRole::Text).unwrap();
        let subtle_text = theme.role(ThemeRole::SubtleText).unwrap();
        let line = theme.role(ThemeRole::Line).unwrap();

        assert!(
            contrast_ratio(text, canvas) >= 4.5,
            "{} text must remain readable on its canvas",
            preset.id()
        );
        assert!(
            contrast_ratio(subtle_text, canvas) >= 4.5,
            "{} subtle text must remain readable on its canvas",
            preset.id()
        );
        assert!(
            contrast_ratio(line, canvas) >= 3.0,
            "{} structural lines must remain distinguishable from the canvas",
            preset.id()
        );
    }
}

fn contrast_ratio(foreground: &str, background: &str) -> f64 {
    let foreground = relative_luminance(foreground);
    let background = relative_luminance(background);
    let (lighter, darker) = if foreground >= background {
        (foreground, background)
    } else {
        (background, foreground)
    };
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(color: &str) -> f64 {
    let color = color
        .strip_prefix('#')
        .expect("bundled theme representatives must use hex colors");
    assert_eq!(color.len(), 6);
    let channel = |range: std::ops::Range<usize>| {
        let value = u8::from_str_radix(&color[range], 16).unwrap() as f64 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(0..2) + 0.7152 * channel(2..4) + 0.0722 * channel(4..6)
}

#[test]
fn explicit_light_appearance_and_quoted_font_family_are_real_theme_inputs() {
    let theme = HostTheme::new()
        .with_appearance(merman_render::presentation::HostThemeAppearance::Light)
        .try_with_font_family(r#"Inter, "Segoe UI", sans-serif"#)
        .expect("quoted CSS font names should be accepted");
    let config = effective_config(
        Presentation::new().with_theme(theme),
        "sequenceDiagram\nAlice->>Bob: Hello",
    );

    assert_eq!(config["theme"], "base");
    assert_eq!(config["darkMode"], false);
    assert_eq!(config["fontFamily"], r#"Inter, "Segoe UI", sans-serif"#);
}

#[test]
fn custom_semantic_role_overrides_preset_data() {
    let theme = HostTheme::from_preset(HostThemePreset::OneDark)
        .try_with_role(ThemeRole::Canvas, "#010203")
        .expect("safe CSS color should be accepted");
    let config = effective_config(
        Presentation::new().with_theme(theme),
        "sequenceDiagram\nAlice->>Bob: Hello",
    );

    assert_eq!(config["theme"], "base");
    assert_eq!(config["themeVariables"]["background"], "#010203");
    assert_eq!(config["themeVariables"]["lineColor"], "#61afef");
}

#[test]
fn explicit_theme_overrides_modern_theme_defaults_without_erasing_profile_aspects() {
    let config = effective_config(
        Presentation::new()
            .with_profile(PresentationProfile::MermanModern)
            .with_theme(HostTheme::from_preset(HostThemePreset::OneDark)),
        "flowchart TD\nA-->B",
    );

    assert_eq!(config["theme"], "base");
    assert_eq!(config["themeVariables"]["background"], "#282c34");
    assert_eq!(config["look"], "neo");
    assert_eq!(config["flowchart"]["layout"], "elk");
    assert_eq!(config["layout"], "elk");
    assert!(config["flowchart"].get("edgeCornerRadius").is_none());
    assert!(config["flowchart"].get("edgeLabelPadding").is_none());
    assert!(config["flowchart"].get("compactEdgeCorners").is_none());
}

#[test]
fn presentation_profile_catalog_describes_independent_aspects() {
    let profiles = presentation_profile_descriptors();

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id(), "merman-modern");
    assert_eq!(
        profiles[0]
            .aspects()
            .iter()
            .map(|aspect| aspect.id())
            .collect::<Vec<_>>(),
        ["global-defaults", "flowchart-svg", "flowchart-elk-default"]
    );
    assert_eq!(
        profiles[0].aspects()[2].required_capability_id(),
        Some("layout-elk")
    );
}

#[test]
fn unknown_ids_and_unsafe_css_fail_closed() {
    assert!(PresentationProfile::from_id("future-profile").is_err());
    assert!(HostThemePreset::from_id("merman-modern").is_err());
    assert!(
        HostTheme::new()
            .try_with_role(ThemeRole::Canvas, "white; color: red")
            .is_err()
    );
}
