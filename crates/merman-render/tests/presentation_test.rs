use merman_core::Engine;
use merman_render::presentation::{
    HostTheme, HostThemePreset, PresentationProfile, ResolvedPresentation, ThemeRole,
    presentation_profile_descriptors, theme_preset_descriptors,
};

fn effective_config(
    profile: Option<PresentationProfile>,
    theme: Option<HostTheme>,
    later_mermaid_theme_selected: bool,
    source: &str,
) -> serde_json::Value {
    ResolvedPresentation::resolve(profile, theme.as_ref(), later_mermaid_theme_selected)
        .materialize_engine(Engine::new())
        .parse_metadata_sync(source)
        .expect("metadata parse should succeed")
        .effective_config
        .as_value()
        .clone()
}

#[test]
fn omitted_presentation_inputs_preserve_default_effective_config() {
    let source = "sequenceDiagram\nAlice->>Bob: Hello";
    let baseline = Engine::new()
        .parse_metadata_sync(source)
        .expect("baseline metadata should parse")
        .effective_config
        .as_value()
        .clone();

    assert_eq!(effective_config(None, None, false, source), baseline);
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
        None,
        Some(theme),
        false,
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
        None,
        Some(theme),
        false,
        "sequenceDiagram\nAlice->>Bob: Hello",
    );

    assert_eq!(config["theme"], "base");
    assert_eq!(config["themeVariables"]["background"], "#010203");
    assert_eq!(config["themeVariables"]["lineColor"], "#61afef");
}

#[test]
fn explicit_theme_overrides_modern_theme_defaults_without_erasing_profile_aspects() {
    let config = effective_config(
        Some(PresentationProfile::MermanModern),
        Some(HostTheme::from_preset(HostThemePreset::OneDark)),
        false,
        "flowchart TD\nA-->B",
    );

    assert_eq!(config["theme"], "base");
    assert_eq!(config["themeVariables"]["background"], "#282c34");
    assert_eq!(config["look"], "neo");
    assert_eq!(config["flowchart"]["defaultRenderer"], "elk");
    assert!(config["flowchart"].get("edgeCornerRadius").is_none());
    assert!(config["flowchart"].get("edgeLabelPadding").is_none());
    assert!(config["flowchart"].get("compactEdgeCorners").is_none());
}

#[test]
fn modern_profile_uses_its_visual_fallback_only_without_a_later_theme_owner() {
    let fallback = effective_config(
        Some(PresentationProfile::MermanModern),
        None,
        false,
        "flowchart TD\nA-->B",
    );
    assert_eq!(fallback["theme"], "redux");
    assert_eq!(fallback["themeVariables"]["mainBkg"], "#F8FAFC");

    let explicit_mermaid_theme = effective_config(
        Some(PresentationProfile::MermanModern),
        None,
        true,
        "flowchart TD\nA-->B",
    );
    assert_eq!(explicit_mermaid_theme["theme"], "default");
    assert_eq!(
        explicit_mermaid_theme["themeVariables"]["mainBkg"],
        "#ECECFF"
    );
    assert_ne!(
        explicit_mermaid_theme["themeVariables"]["nodeBorder"],
        "#64748B"
    );
    assert_eq!(explicit_mermaid_theme["look"], "neo");
    assert_eq!(
        explicit_mermaid_theme["flowchart"]["defaultRenderer"],
        "elk"
    );
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
