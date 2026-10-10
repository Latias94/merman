#![cfg(feature = "diagram-packet")]

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, GradientStop,
    LinearGradient, MermaidThemeCompatibility, OrdinalSelector, PatternKind, PatternSpec,
    Specified, ThemeColorValue, ThemePortabilityRequirement, ThemePreset, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::PacketDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};

fn packet_font_theme(font_family: &str) -> DiagramTheme {
    packet_font_stack_theme(
        FontStack::single(font_family).expect("valid Packet fixture font family"),
    )
}

fn packet_font_stack_theme(font_stack: FontStack) -> DiagramTheme {
    let typography = ThemeTextStyle::default().with_font_stack(font_stack);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::PACKET, typography),
        ))
        .expect("compile Packet font theme")
}

fn packet_font_size_theme(font_size_px: f32) -> DiagramTheme {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(font_size_px)
        .expect("valid Packet fixture font size");
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::PACKET, typography),
        ))
        .expect("compile Packet font size theme")
}

fn packet_text_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::PACKET),
                ),
            ),
        )
        .expect("compile Packet text fill theme")
}

fn packet_byte_label_and_title_fill_theme(
    byte_label_fill: CanvasPaint,
    title_fill: CanvasPaint,
) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::PacketByteLabel,
                            ThemeStylePatch::default().with_fill(byte_label_fill),
                        )
                        .for_family(DiagramFamilyId::PACKET),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(title_fill),
                        )
                        .for_family(DiagramFamilyId::PACKET),
                    ),
            ),
        )
        .expect("compile Packet byte-label and title fill theme")
}

fn packet_direct_text_theme_with_mermaid(compatibility: MermaidThemeCompatibility) -> DiagramTheme {
    let typography = ThemeTextStyle::default().with_font_stack(
        FontStack::single("monospace").expect("valid Packet compatibility font stack"),
    );
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_mermaid_compatibility(compatibility)
                .with_typography(
                    TypographySpec::default()
                        .with_family_style(DiagramFamilyId::PACKET, typography),
                )
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::PacketByteLabel,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid("#e5e7eb")
                                        .expect("valid Packet compatibility byte-label fill"),
                                ),
                            )
                            .for_family(DiagramFamilyId::PACKET),
                        )
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::PacketFieldLabel,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid("#111827")
                                        .expect("valid Packet compatibility field-label fill"),
                                ),
                            )
                            .for_family(DiagramFamilyId::PACKET),
                        )
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Title,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid("#fef3c7")
                                        .expect("valid Packet compatibility title fill"),
                                ),
                            )
                            .for_family(DiagramFamilyId::PACKET),
                        ),
                ),
        )
        .expect("compile Packet compatibility theme")
}

fn packet_ordinal_fill_theme(target: ThemeTarget, exact: usize) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#123456").expect("valid Packet ordinal fill"),
                        ),
                    )
                    .for_family(DiagramFamilyId::PACKET)
                    .with_ordinal(
                        OrdinalSelector::exact(exact).expect("valid Packet exact ordinal"),
                    ),
                ),
            ),
        )
        .expect("compile Packet ordinal fill theme")
}

fn packet_variant_ordinal_fill_theme(
    target: ThemeTarget,
    variant: ThemeVariant,
    exact: usize,
) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#123456")
                                .expect("valid Packet variant ordinal fill"),
                        ),
                    )
                    .for_family(DiagramFamilyId::PACKET)
                    .with_variant(variant)
                    .with_ordinal(
                        OrdinalSelector::exact(exact).expect("valid Packet exact ordinal"),
                    ),
                ),
            ),
        )
        .expect("compile Packet variant ordinal fill theme")
}

fn packet_source() -> &'static str {
    r#"packet
title Packet Typography
0-7: "Header"
8-15: "Payload"
"#
}

fn render_packet_with_theme_and_engine(
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
) -> family::RenderedFamilySvg {
    try_render_packet_with_theme_and_engine(theme, engine, diagram_id)
        .expect("render themed Packet")
}

fn try_render_packet_with_theme_and_engine(
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_packet_source_with_theme_and_engine(packet_source(), theme, engine, diagram_id)
}

fn try_render_packet_source_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Packet")
        .expect("detect themed Packet");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Packet session");

    family::prepare(parsed, &LayoutOptions::default(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn packet_stylesheet<'input>(document: &'input roxmltree::Document<'input>) -> &'input str {
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Packet stylesheet")
}

#[test]
fn packet_byte_label_and_title_fill_reach_only_their_terminal_roles() {
    let theme = packet_byte_label_and_title_fill_theme(
        CanvasPaint::solid("#e5e7eb").expect("valid Packet byte-label fill"),
        CanvasPaint::solid("#fef3c7").expect("valid Packet title fill"),
    );
    let rendered =
        render_packet_with_theme_and_engine(&theme, Engine::new(), "packet-terminal-text-fill");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Packet SVG");
    let css = packet_stylesheet(&document);

    assert!(css.contains("#packet-terminal-text-fill .packetByte.start{fill:#e5e7eb;}"));
    assert!(css.contains("#packet-terminal-text-fill .packetByte.end{fill:#e5e7eb;}"));
    assert!(css.contains("#packet-terminal-text-fill .packetLabel{fill:black;font-size:12px;}"));
    assert!(css.contains("#packet-terminal-text-fill .packetTitle{fill:#fef3c7;font-size:14px;}"));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_explicit_role_colors_outrank_only_their_typed_fill_roles() {
    let theme = packet_byte_label_and_title_fill_theme(
        CanvasPaint::solid("#e5e7eb").expect("valid Packet byte-label fill"),
        CanvasPaint::solid("#fef3c7").expect("valid Packet title fill"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "packet": {
            "startByteColor": "#111111",
            "labelColor": "#222222",
            "titleColor": "#333333"
        }
    })));
    let rendered =
        render_packet_with_theme_and_engine(&theme, engine, "packet-explicit-role-colors");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid configured Packet SVG");
    let css = packet_stylesheet(&document);

    assert!(css.contains("#packet-explicit-role-colors .packetByte.start{fill:#111111;}"));
    assert!(css.contains("#packet-explicit-role-colors .packetByte.end{fill:#e5e7eb;}"));
    assert!(
        css.contains("#packet-explicit-role-colors .packetLabel{fill:#222222;font-size:12px;}")
    );
    assert!(
        css.contains("#packet-explicit-role-colors .packetTitle{fill:#333333;font-size:14px;}")
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_dark_presets_pair_each_text_role_with_its_terminal_background() {
    let cases = [
        (ThemePreset::EditorDark, "#e5e7eb", "#0f172a"),
        (ThemePreset::OneDark, "#abb2bf", "#282c34"),
        (ThemePreset::GruvboxDark, "#ebdbb2", "#282828"),
        (ThemePreset::AyuDark, "#bfbdb6", "#0b0e14"),
    ];

    for (preset, expected_dark_surface_fill, expected_field_label_fill) in cases {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset)
            .expect("compile dark Packet preset");
        let metadata = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_metadata_sync(packet_source())
            .expect("parse dark Packet preset metadata");
        let parse_evidence = merman_core::__private::theme_parse_evidence(&metadata);
        // Retained dark presets expose Mermaid's compatibility theme name. Their typed palette
        // owns the derived colors, so they intentionally do not claim Mermaid darkMode paths.
        assert_eq!(parse_evidence.mermaid_residual_count(), 1);
        assert!(parse_evidence.mermaid_theme_field_survives());
        assert!(!parse_evidence.mermaid_dark_mode_field_survives());
        assert!(parse_evidence.mermaid_residual_path_survives("theme"));
        assert!(!parse_evidence.mermaid_residual_path_survives("darkMode"));
        assert!(!parse_evidence.mermaid_residual_path_survives("themeVariables.darkMode"));
        assert!(!parse_evidence.mermaid_residual_path_survives("themeVariables.primaryColor"));

        let diagram_id = format!("packet-{}", preset.id());
        let rendered = render_packet_with_theme_and_engine(&theme, Engine::new(), &diagram_id);
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid preset Packet SVG");
        let css = packet_stylesheet(&document);

        for expected_rule in [
            format!("#{diagram_id} .packetByte.start{{fill:{expected_dark_surface_fill};}}"),
            format!("#{diagram_id} .packetByte.end{{fill:{expected_dark_surface_fill};}}"),
            format!(
                "#{diagram_id} .packetTitle{{fill:{expected_dark_surface_fill};font-size:14px;}}"
            ),
        ] {
            assert!(
                css.contains(&expected_rule),
                "{preset:?} must brighten Packet text drawn on the dark canvas: {css}"
            );
        }
        assert!(
            css.contains(&format!(
                "#{diagram_id} .packetLabel{{fill:{expected_field_label_fill};font-size:12px;}}"
            )),
            "{preset:?} must use its Packet-specific dark-on-light field label: {css}"
        );
        assert!(
            css.contains(&format!(
                "#{diagram_id} .packetBlock{{stroke:black;stroke-width:1;fill:#efefef;}}"
            )),
            "{preset:?} field-label proof requires the light Packet block baseline: {css}"
        );

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 5);
        assert_eq!(evidence.accounted_count(), 5);
        assert_eq!(evidence.applied_count(), 4);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
        assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
    }
}

#[test]
fn packet_consumes_one_canonical_dark_mode_concept_when_only_one_path_survives() {
    let compatibility = MermaidThemeCompatibility::default()
        .with_theme("base")
        .expect("valid Packet compatibility theme")
        .with_dark_mode(true)
        .expect("valid Packet compatibility dark mode");
    let theme = packet_direct_text_theme_with_mermaid(compatibility);
    let cases = [
        (
            "root-owned",
            serde_json::json!({ "darkMode": true }),
            false,
            true,
        ),
        (
            "variable-owned",
            serde_json::json!({ "themeVariables": { "darkMode": true } }),
            true,
            false,
        ),
    ];

    for (case, site_config, root_survives, variable_survives) in cases {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(site_config));
        let metadata =
            merman_render::__private::install_parse_compatibility(&theme, engine.clone())
                .parse_metadata_sync(packet_source())
                .expect("parse one-sided Packet dark-mode ownership");
        let parse_evidence = merman_core::__private::theme_parse_evidence(&metadata);

        assert_eq!(parse_evidence.mermaid_residual_count(), 2, "{case}");
        assert!(parse_evidence.mermaid_theme_field_survives(), "{case}");
        assert!(parse_evidence.mermaid_dark_mode_field_survives(), "{case}");
        assert_eq!(
            parse_evidence.mermaid_residual_path_survives("darkMode"),
            root_survives,
            "{case}"
        );
        assert_eq!(
            parse_evidence.mermaid_residual_path_survives("themeVariables.darkMode"),
            variable_survives,
            "{case}"
        );

        let diagram_id = format!("packet-one-sided-dark-mode-{case}");
        let rendered = render_packet_with_theme_and_engine(&theme, engine, &diagram_id);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.mermaid_compatibility_residual_count(), 0, "{case}");
    }
}

#[test]
fn packet_custom_recipe_reconciles_fields_by_terminal_consumption_not_preset_shape() {
    let compatibility = MermaidThemeCompatibility::default()
        .with_theme("dark")
        .expect("valid non-preset Packet compatibility theme");
    let theme = packet_direct_text_theme_with_mermaid(compatibility);
    let rendered = try_render_packet_with_theme_and_engine(
        &theme,
        Engine::new(),
        "packet-custom-compatibility-reconciliation",
    )
    .expect("custom Packet recipes use the same conceptual-field reconciliation as presets");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_direct_compatibility_deferral_fails_closed_without_terminal_font_coverage() {
    let compatibility = MermaidThemeCompatibility::default()
        .with_theme("base")
        .expect("valid Packet compatibility theme")
        .with_dark_mode(true)
        .expect("valid Packet compatibility dark mode");
    let theme = packet_direct_text_theme_with_mermaid(compatibility);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "fontFamily": "Courier, monospace"
    })));
    let error = try_render_packet_with_theme_and_engine(
        &theme,
        engine,
        "packet-config-owned-terminal-font",
    )
    .err()
    .expect("provisional compatibility deferral must be revoked without terminal typed coverage");

    assert!(matches!(
        error,
        merman_render::Error::MermaidThemeCompatibility {
            family_id: DiagramFamilyId::PACKET,
            residual_count: 2,
        }
    ));
}

#[test]
fn packet_does_not_retire_unrelated_mermaid_compatibility_fields() {
    let compatibility = MermaidThemeCompatibility::default()
        .with_theme("base")
        .expect("valid Packet compatibility theme")
        .with_dark_mode(true)
        .expect("valid Packet compatibility dark mode")
        .with_variable("primaryColor", "#123456")
        .expect("valid unrelated Packet compatibility variable");
    let theme = packet_direct_text_theme_with_mermaid(compatibility);
    let error = try_render_packet_with_theme_and_engine(
        &theme,
        Engine::new(),
        "packet-unrelated-mermaid-compatibility",
    )
    .err()
    .expect("strict Packet rendering must retain unrelated Mermaid compatibility");

    assert!(matches!(
        error,
        merman_render::Error::MermaidThemeCompatibility {
            family_id: DiagramFamilyId::PACKET,
            residual_count: 1,
        }
    ));
}

#[test]
fn packet_keeps_dark_mode_residual_when_one_sided_values_are_not_canonical() {
    let compatibility = MermaidThemeCompatibility::default()
        .with_theme("base")
        .expect("valid Packet compatibility theme")
        .with_dark_mode(true)
        .expect("valid Packet compatibility dark mode");
    let theme = packet_direct_text_theme_with_mermaid(compatibility);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "darkMode": false
    })));
    let error =
        try_render_packet_with_theme_and_engine(&theme, engine, "packet-noncanonical-dark-mode")
            .err()
            .expect("non-canonical mirrored dark-mode values must remain a strict residual");

    assert!(matches!(
        error,
        merman_render::Error::MermaidThemeCompatibility {
            family_id: DiagramFamilyId::PACKET,
            residual_count: 1,
        }
    ));
}

#[test]
fn packet_visible_text_rejects_unsupported_base_font_size() {
    let theme = packet_font_size_theme(18.0);
    let error = try_render_packet_with_theme_and_engine(
        &theme,
        Engine::new(),
        "packet-unsupported-font-size",
    )
    .err()
    .expect("strict portability must reject unsupported Packet base font size");

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::PACKET, 1))
    );
    assert_eq!(error.incomplete_family_theme(), None);
}

#[test]
fn packet_generic_text_fill_reaches_visible_field_labels() {
    let theme = packet_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid empty Packet text fill"),
    );
    let rendered = try_render_packet_source_with_theme_and_engine(
        "packet\n0-7: \"Header\"\n",
        &theme,
        Engine::new(),
        "packet-generic-text-fill",
    )
    .expect("generic Text must inherit into the Packet field-label role");
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid generic-text Packet SVG");
    let css = packet_stylesheet(&document);
    assert!(css.contains("#packet-generic-text-fill .packetLabel{fill:#123456;font-size:12px;}"));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_source_label_color_outranks_generic_text_fill_at_css_and_dom_receipt() {
    let theme = packet_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Packet generic text fill"),
    );
    let rendered = try_render_packet_source_with_theme_and_engine(
        r##"%%{init: {"packet": {"labelColor": "#abcdef"}}}%%
packet
0-7: "Header"
"##,
        &theme,
        Engine::new(),
        "packet-source-label-color",
    )
    .expect("source-owned Packet labelColor must suppress the typed field-label fill");
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid source-owned Packet SVG");
    let css = packet_stylesheet(&document);
    assert!(css.contains("#packet-source-label-color .packetLabel{fill:#abcdef;font-size:12px;}"));
    assert!(!css.contains("#packet-source-label-color .packetLabel{fill:#123456;"));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_site_label_color_outranks_generic_text_fill_at_css_and_dom_receipt() {
    let theme = packet_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Packet generic text fill"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "packet": {
            "labelColor": "#abcdef"
        }
    })));
    let rendered = try_render_packet_source_with_theme_and_engine(
        "packet\n0-7: \"Header\"\n",
        &theme,
        engine,
        "packet-site-label-color",
    )
    .expect("site-owned Packet labelColor must suppress the typed field-label fill");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid site-owned Packet SVG");
    let css = packet_stylesheet(&document);
    assert!(css.contains("#packet-site-label-color .packetLabel{fill:#abcdef;font-size:12px;}"));
    assert!(!css.contains("#packet-site-label-color .packetLabel{fill:#123456;"));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_byte_label_ordinal_counts_each_visible_byte_occurrence() {
    let theme = packet_ordinal_fill_theme(ThemeTarget::PacketByteLabel, 2);
    let error = try_render_packet_with_theme_and_engine(
        &theme,
        Engine::new(),
        "packet-byte-label-exact-two",
    )
    .err()
    .expect("the second visible Packet byte label must keep the unsupported route residual");

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::PACKET, 1))
    );
    assert_eq!(error.incomplete_family_theme(), None);
}

#[test]
fn packet_title_ordinal_is_not_applicable_when_only_body_text_is_emitted() {
    let theme = packet_ordinal_fill_theme(ThemeTarget::Title, 1);
    let rendered = try_render_packet_source_with_theme_and_engine(
        "packet\n0-7: \"Header\"\n",
        &theme,
        Engine::new(),
        "packet-title-absent",
    )
    .expect("body text must not manufacture a Packet title occurrence");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_non_default_variant_is_not_applicable_even_when_ordinal_matches() {
    let theme =
        packet_variant_ordinal_fill_theme(ThemeTarget::PacketFieldLabel, ThemeVariant::Active, 1);
    let rendered = try_render_packet_source_with_theme_and_engine(
        "packet\n0-7: \"Header\"\n",
        &theme,
        Engine::new(),
        "packet-active-field-label",
    )
    .expect("Packet exposes only the Default field-label variant domain");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid variant Packet SVG");
    let css = packet_stylesheet(&document);
    assert!(css.contains("#packet-active-field-label .packetLabel{fill:black;font-size:12px;}"));
    assert!(!css.contains("#packet-active-field-label .packetLabel{fill:#123456;"));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_owned_field_label_fill_makes_unsupported_fill_kinds_not_applicable() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#0f172a").expect("valid Packet gradient start"),
            )
            .expect("valid Packet gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#22d3ee").expect("valid Packet gradient end"),
            )
            .expect("valid Packet gradient stop"),
        ],
    )
    .expect("valid Packet gradient");
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#22d3ee").expect("valid Packet pattern color"),
    )
    .expect("valid Packet pattern");
    let mut clear = ThemeStylePatch::default();
    clear.paint.fill = Specified::Clear;
    let cases = [
        (
            "gradient",
            ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
            None,
        ),
        (
            "pattern",
            ThemeStylePatch::default().with_fill(CanvasPaint::Pattern(pattern)),
            None,
        ),
        ("clear", clear, None),
        (
            "ordinal",
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#123456").expect("valid Packet ordinal fill")),
            Some(1),
        ),
    ];

    for (case, patch, ordinal) in cases {
        for owner in ["source", "site"] {
            let mut rule = ThemeRule::new(ThemeTarget::PacketFieldLabel, patch.clone())
                .for_family(DiagramFamilyId::PACKET);
            if let Some(ordinal) = ordinal {
                rule = rule.with_ordinal(
                    OrdinalSelector::exact(ordinal).expect("valid Packet field-label ordinal"),
                );
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)),
                )
                .expect("compile config-owned Packet field-label theme");
            let diagram_id = format!("packet-{owner}-owned-{case}");
            let (source, engine) = if owner == "source" {
                (
                    r##"%%{init: {"packet": {"labelColor": "#abcdef"}}}%%
packet
0-7: "Header"
"##,
                    Engine::new(),
                )
            } else {
                (
                    "packet\n0-7: \"Header\"\n",
                    Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                        "packet": { "labelColor": "#abcdef" }
                    }))),
                )
            };
            let rendered = try_render_packet_source_with_theme_and_engine(
                source,
                &theme,
                engine,
                &diagram_id,
            )
            .expect("config-owned Packet fill must make the unsupported fill route not applicable");
            let document =
                roxmltree::Document::parse(rendered.svg()).expect("valid config-owned Packet SVG");
            let css = packet_stylesheet(&document);
            assert!(
                css.contains(&format!(
                    "#{diagram_id} .packetLabel{{fill:#abcdef;font-size:12px;}}"
                )),
                "{owner}/{case}: ownership must reach terminal Packet field-label CSS: {css}"
            );

            drop(document);
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "{owner}/{case}");
            assert_eq!(evidence.accounted_count(), 1, "{owner}/{case}");
            assert_eq!(evidence.applied_count(), 0, "{owner}/{case}");
            assert_eq!(evidence.not_applicable_count(), 1, "{owner}/{case}");
            assert_eq!(evidence.theme_residual_count(), 0, "{owner}/{case}");
        }
    }
}

#[test]
fn packet_owned_field_label_fill_does_not_hide_an_unrelated_winning_facet() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#0f172a").expect("valid Packet gradient start"),
            )
            .expect("valid Packet gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#22d3ee").expect("valid Packet gradient end"),
            )
            .expect("valid Packet gradient stop"),
        ],
    )
    .expect("valid Packet gradient");
    let mut patch = ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient));
    patch.geometry.radius = Specified::Value(8.0);
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(ThemeTarget::PacketFieldLabel, patch)
                        .for_family(DiagramFamilyId::PACKET),
                ),
            ),
        )
        .expect("compile mixed-facet Packet field-label theme");
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "packet": { "labelColor": "#abcdef" }
    })));
    let error = try_render_packet_source_with_theme_and_engine(
        "packet\n0-7: \"Header\"\n",
        &theme,
        engine,
        "packet-owned-fill-mixed-facet",
    )
    .err()
    .expect("config-owned fill must not suppress a winning unsupported geometry facet");

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::PACKET, 1))
    );
}

#[test]
fn packet_late_static_fill_shadows_earlier_ordinal_across_text_inheritance() {
    for (case, ordinal_target, static_target) in [
        (
            "text-to-field",
            ThemeTarget::Text,
            ThemeTarget::PacketFieldLabel,
        ),
        (
            "field-to-text",
            ThemeTarget::PacketFieldLabel,
            ThemeTarget::Text,
        ),
    ] {
        let rules = ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(
                    ordinal_target,
                    ThemeStylePatch::default().with_fill(
                        CanvasPaint::solid("#123456").expect("valid early Packet ordinal fill"),
                    ),
                )
                .for_family(DiagramFamilyId::PACKET)
                .with_ordinal(
                    OrdinalSelector::exact(1).expect("valid early Packet field-label ordinal"),
                ),
            )
            .with_rule(
                ThemeRule::new(
                    static_target,
                    ThemeStylePatch::default().with_fill(
                        CanvasPaint::solid("#abcdef").expect("valid late Packet static fill"),
                    ),
                )
                .for_family(DiagramFamilyId::PACKET),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .expect("compile shadowed Packet ordinal theme");
        let diagram_id = format!("packet-shadowed-ordinal-{case}");
        let rendered = try_render_packet_source_with_theme_and_engine(
            "packet\n0-7: \"Header\"\n",
            &theme,
            Engine::new(),
            &diagram_id,
        )
        .expect("a later static field-label fill must retire the shadowed ordinal residual");
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid shadowed-ordinal Packet SVG");
        let css = packet_stylesheet(&document);
        assert!(
            css.contains(&format!(
                "#{diagram_id} .packetLabel{{fill:#abcdef;font-size:12px;}}"
            )),
            "{case}: the final static winner must reach terminal Packet CSS: {css}"
        );
        assert!(!css.contains(".packetLabel{fill:#123456;"), "{case}");

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2, "{case}");
        assert_eq!(evidence.accounted_count(), 2, "{case}");
        assert_eq!(evidence.applied_count(), 1, "{case}");
        assert_eq!(evidence.not_applicable_count(), 1, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn packet_typed_font_stack_reaches_root_css_and_visible_text_inherits_it() {
    let theme = packet_font_theme("monospace");
    let rendered = render_packet_with_theme_and_engine(&theme, Engine::new(), "packet-theme");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Packet SVG");
    let css = packet_stylesheet(&document);

    assert!(
        css.contains("#packet-theme{font-family:monospace;font-size:16px;fill:#333;}"),
        "typed Packet font must reach the root stylesheet: {css}"
    );
    assert!(
        css.contains("#packet-theme svg{font-family:monospace;font-size:16px;}"),
        "typed Packet font must reach the nested SVG base rule: {css}"
    );
    assert!(
        css.contains("#packet-theme :root{--mermaid-font-family:monospace;}"),
        "typed Packet font must reach the Mermaid root variable: {css}"
    );
    assert!(css.contains("#packet-theme .packetByte{font-size:10px;}"));
    assert!(css.contains("#packet-theme .packetLabel{fill:black;font-size:12px;}"));
    assert!(css.contains("#packet-theme .packetTitle{fill:black;font-size:14px;}"));

    let visible_text = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text") && node.text().is_some_and(|text| !text.trim().is_empty())
        })
        .collect::<Vec<_>>();
    assert!(
        !visible_text.is_empty(),
        "Packet fixture must emit visible text"
    );
    for text in visible_text {
        let classes = text.attribute("class").expect("Packet text role class");
        assert!(
            classes
                .split_ascii_whitespace()
                .any(|class| { matches!(class, "packetLabel" | "packetByte" | "packetTitle") }),
            "unexpected Packet text role {classes:?}"
        );
        assert_eq!(text.attribute("font-family"), None);
        assert_eq!(text.attribute("style"), None);
    }

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

#[test]
fn packet_explicit_mermaid_font_paths_outrank_typed_font_stack() {
    let theme = packet_font_theme("monospace");
    let cases = [
        (
            "packet-root-font",
            serde_json::json!({ "fontFamily": "Courier, monospace" }),
            "Courier,monospace",
        ),
        (
            "packet-theme-variable-font",
            serde_json::json!({
                "themeVariables": { "fontFamily": "serif" }
            }),
            "serif",
        ),
    ];

    for (diagram_id, site_config, expected_font) in cases {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(site_config));
        let rendered = render_packet_with_theme_and_engine(&theme, engine, diagram_id);
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid configured Packet SVG");
        let css = packet_stylesheet(&document);
        assert!(
            css.contains(&format!(
                "#{diagram_id}{{font-family:{expected_font};font-size:16px;fill:#333;}}"
            )),
            "explicit Mermaid font must own Packet root CSS: {css}"
        );
        assert!(
            css.contains(&format!(
                "#{diagram_id} svg{{font-family:{expected_font};font-size:16px;}}"
            )),
            "explicit Mermaid font must own Packet nested SVG CSS: {css}"
        );
        assert!(!css.contains(&format!("#{diagram_id}{{font-family:monospace;")));

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
        assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
    }
}

#[test]
fn packet_role_style_values_cannot_override_the_typed_font_stack() {
    let theme = packet_font_theme("monospace");
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "packet": {
            "labelColor": "red;font-family:serif"
        }
    })));
    let rendered = render_packet_with_theme_and_engine(&theme, engine, "packet-safe-role-style");
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid safely configured Packet SVG");
    let css = packet_stylesheet(&document);

    assert!(css.contains("#packet-safe-role-style .packetLabel{fill:black;font-size:12px;}"));
    assert!(!css.contains("font-family:serif"));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_direct_font_stack_is_not_limited_by_the_legacy_bridge_budget() {
    let families = (0..32)
        .map(|index| format!("PacketFont{index:02}{}", "x".repeat(140)))
        .collect::<Vec<_>>();
    let font_stack = FontStack::new(families).expect("valid maximum-width Packet font stack");
    let expected_css = font_stack.as_css();
    assert!(expected_css.len() > 4 * 1024);

    let theme = packet_font_stack_theme(font_stack);
    let rendered =
        render_packet_with_theme_and_engine(&theme, Engine::new(), "packet-large-font-stack");
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid large-stack Packet SVG");
    let css = packet_stylesheet(&document);

    assert!(css.contains(&format!(
        "#packet-large-font-stack{{font-family:{expected_css};font-size:16px;fill:#333;}}"
    )));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

const DESCENDING_PACKET: &str = include_str!("../../../fixtures/packet/bit_order_descending.mmd");

fn render_packet(source: &str) -> (PacketDiagramLayout, String) {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse packet")
        .expect("packet diagram");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(parsed, &LayoutOptions::default(), session).expect("packet layout");
    let projection = artifact.layout_json().expect("layout projection");
    let layout = serde_json::from_value(projection["layout"]["PacketDiagram"].clone())
        .expect("packet layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("packet SVG")
        .svg()
        .to_owned();
    (layout, svg)
}

#[test]
fn packet_descending_mirrors_each_row_without_reversing_semantic_ranges() {
    let (ascending, _) = render_packet(&DESCENDING_PACKET.replace("descending", "ascending"));
    let (descending, svg) = render_packet(DESCENDING_PACKET);
    assert_eq!(descending.words.len(), 2);
    assert_eq!(
        (ascending.width, ascending.height),
        (descending.width, descending.height)
    );
    for (before, after) in ascending.words.iter().zip(&descending.words) {
        for (before, after) in before.blocks.iter().zip(&after.blocks) {
            assert_eq!(
                (before.start, before.end, &before.label),
                (after.start, after.end, &after.label)
            );
            assert_eq!(
                (before.width, before.height, before.y),
                (after.width, after.height, after.y)
            );
            let bits = after.end - after.start + 1;
            assert_eq!(after.x, 1.0 + (16 - after.start % 16 - bits) as f64 * 10.0);
        }
    }
    assert_eq!(
        descending.words[0]
            .blocks
            .iter()
            .map(|block| block.x)
            .collect::<Vec<_>>(),
        [81.0, 41.0, 31.0, 1.0]
    );
    assert_eq!(
        descending.words[1]
            .blocks
            .iter()
            .map(|block| block.x)
            .collect::<Vec<_>>(),
        [111.0, 81.0]
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let labels = |class| {
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some(class))
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        labels("packetByte start"),
        ["7", "11", "12", "15", "20", "23"]
    );
    assert_eq!(labels("packetByte end"), ["0", "8", "13", "16", "21"]);
    let single = document
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("packetByte start") && node.text() == Some("12")
        })
        .unwrap();
    assert_eq!(single.attribute("text-anchor"), Some("middle"));
    assert_eq!(single.attribute("x"), Some("35"));
    assert_eq!(
        labels("packetLabel"),
        ["DATA", "TYPE", "EN", "FLAGS", "FLAGS", "TAIL"]
    );
}

#[test]
fn packet_descending_applies_when_bit_numbers_are_hidden() {
    let (visible, _) = render_packet(DESCENDING_PACKET);
    let (hidden, svg) =
        render_packet(&DESCENDING_PACKET.replace("showBits: true", "showBits: false"));
    assert!(!hidden.show_bits);
    for (visible, hidden) in visible.words.iter().zip(&hidden.words) {
        for (visible, hidden) in visible.blocks.iter().zip(&hidden.blocks) {
            assert_eq!((visible.x, visible.width), (hidden.x, hidden.width));
        }
    }
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(!document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.starts_with("packetByte "))
    }));
}

#[test]
fn packet_default_order_is_ascending_and_one_bit_rows_keep_absolute_numbers() {
    let (ascending, explicit) =
        render_packet(&DESCENDING_PACKET.replace("descending", "ascending"));
    let (default, implicit) =
        render_packet(&DESCENDING_PACKET.replace("    bitOrder: descending", ""));
    assert_eq!(
        serde_json::to_value(&ascending).unwrap(),
        serde_json::to_value(&default).unwrap()
    );
    assert_eq!(explicit, implicit);

    let (layout, svg) = render_packet(
        "---\nconfig:\n  packet:\n    bitsPerRow: 1\n    bitOrder: descending\n---\npacket\n+3: \"bits\"\n",
    );
    assert_eq!(layout.words.len(), 3);
    assert!(layout.words.iter().all(|word| word.blocks[0].x == 1.0));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let leading: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("packetByte start"))
        .map(|node| node.text().unwrap())
        .collect();
    assert_eq!(leading, ["0", "1", "2"]);
    assert!(
        !document
            .descendants()
            .any(|node| node.attribute("class") == Some("packetByte end"))
    );
}
