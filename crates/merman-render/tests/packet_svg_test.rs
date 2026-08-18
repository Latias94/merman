use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalSelector,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
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

fn packet_ordinal_text_fill_theme(target: ThemeTarget, exact: usize) -> DiagramTheme {
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
        .expect("compile Packet ordinal text theme")
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
fn packet_visible_text_fill_is_an_explicit_unsupported_residual() {
    let theme =
        packet_text_fill_theme(CanvasPaint::solid("#123456").expect("valid Packet text fill"));
    let error = try_render_packet_with_theme_and_engine(
        &theme,
        Engine::new(),
        "packet-unsupported-text-fill",
    )
    .err()
    .expect("strict portability must reject unsupported Packet text fill");

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::PACKET, 1))
    );
    assert_eq!(error.incomplete_family_theme(), None);
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
fn packet_text_fill_is_not_applicable_without_visible_text() {
    let theme = packet_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid empty Packet text fill"),
    );
    let rendered = try_render_packet_source_with_theme_and_engine(
        "packet",
        &theme,
        Engine::new(),
        "packet-empty-text-fill",
    )
    .expect("an absent Packet text target must remain portable");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn packet_text_ordinal_counts_each_visible_body_text_occurrence() {
    let theme = packet_ordinal_text_fill_theme(ThemeTarget::Text, 2);
    let error = try_render_packet_with_theme_and_engine(
        &theme,
        Engine::new(),
        "packet-text-exact-two",
    )
    .err()
    .expect("the second visible Packet text occurrence must keep the unsupported route residual");

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::PACKET, 1))
    );
    assert_eq!(error.incomplete_family_theme(), None);
}

#[test]
fn packet_title_ordinal_is_not_applicable_when_only_body_text_is_emitted() {
    let theme = packet_ordinal_text_fill_theme(ThemeTarget::Title, 1);
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
