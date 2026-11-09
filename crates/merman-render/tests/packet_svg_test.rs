#![cfg(feature = "diagram-packet")]

use merman_core::{MermaidEngine, ParseOptions};
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

fn independent_packet_presentation(
    packet: Vec<Vec<merman_model::families::packet::PacketBlock>>,
    config: serde_json::Value,
) -> merman_model::presentation::PresentationDiagram {
    use merman_model::families::packet::PacketDiagramModel;
    use merman_model::presentation::{
        AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
    };
    use merman_model::resources::InputResourcePolicy;
    use merman_model::{MermaidConfig, OperationControl};

    let config = MermaidConfig::from_value(config);
    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "packet".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
            theme_recipe: None,
        },
        RenderSemanticModel::Packet(PacketDiagramModel {
            packet,
            ..Default::default()
        }),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("independent Packet construction is active")
    .expect("small Packet model passes construction resource admission")
}

#[test]
fn packet_direct_input_rejects_impossible_block_ranges() {
    use merman_model::families::packet::PacketBlock;

    for (start, end) in [
        (-1, 0),
        (1, 0),
        (0, i64::MAX),
        (i64::MIN, i64::MAX),
        (31, 32),
    ] {
        for order in ["ascending", "descending"] {
            let parsed = independent_packet_presentation(
                vec![vec![PacketBlock {
                    start,
                    end,
                    bits: 1,
                    label: "Field".to_string(),
                }]],
                serde_json::json!({"packet": {"bitOrder": order}}),
            );
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let error = family::prepare(&parsed, &LayoutOptions::default(), session)
                .err()
                .expect("impossible Packet ranges must return an error instead of overflowing");
            assert!(matches!(error, merman_render::Error::InvalidModel { .. }));
        }
    }
}

#[test]
fn packet_direct_input_preserves_split_rows_and_single_bit_fields() {
    use merman_model::families::packet::PacketBlock;

    for order in ["ascending", "descending"] {
        let block = |start, end| PacketBlock {
            start,
            end,
            bits: end - start + 1,
            label: format!("{start}-{end}"),
        };
        let parsed = independent_packet_presentation(
            vec![vec![block(0, 7)], vec![block(8, 11), block(12, 12)]],
            serde_json::json!({"packet": {"bitsPerRow": 8, "bitOrder": order}}),
        );
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
            .expect("source-shaped split rows remain valid in both bit orders");
        let projection = artifact.layout_json().unwrap();
        let layout: PacketDiagramLayout =
            serde_json::from_value(projection["layout"]["PacketDiagram"].clone()).unwrap();
        assert_eq!(layout.words.len(), 2);
        let blocks: Vec<_> = layout.words.iter().flat_map(|word| &word.blocks).collect();
        assert_eq!(
            blocks
                .iter()
                .map(|block| (block.start, block.end))
                .collect::<Vec<_>>(),
            [(0, 7), (8, 11), (12, 12)]
        );
        assert_eq!(
            blocks.iter().map(|block| block.width).collect::<Vec<_>>(),
            [251.0, 123.0, 27.0]
        );
        assert_eq!(
            blocks.iter().map(|block| block.x).collect::<Vec<_>>(),
            if order == "ascending" {
                [1.0, 1.0, 129.0]
            } else {
                [1.0, 129.0, 97.0]
            }
        );
    }
}

#[test]
fn packet_direct_input_validation_preserves_operation_error_priority() {
    use merman_model::OperationControl;
    use merman_model::families::packet::PacketBlock;
    use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};

    let parsed = independent_packet_presentation(
        vec![vec![PacketBlock {
            start: 0,
            end: i64::MAX,
            bits: 1,
            label: "Field".to_string(),
        }]],
        serde_json::json!({}),
    );
    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(merman_render::Error::Cancelled(_))
    ));

    let control = OperationControl::new();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxModelItems, 1)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session_with_control(control.clone())
        .unwrap();
    let error = family::plan_render(&parsed, &session).err().unwrap();
    let merman_render::Error::ResourceLimitExceeded(first) = error else {
        panic!("expected model resource admission before Packet validation, got {error:?}");
    };
    assert_eq!(first.limit, ResourceLimitId::MaxModelItems.as_str());
    control.cancel();
    let error = family::prepare(&parsed, &LayoutOptions::default(), session)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        merman_render::Error::ResourceLimitExceeded(replayed) if replayed == first
    ));
}

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
    engine: MermaidEngine,
    diagram_id: &str,
) -> family::RenderedFamilySvg {
    try_render_packet_with_theme_and_engine(theme, engine, diagram_id)
        .expect("render themed Packet")
}

fn try_render_packet_with_theme_and_engine(
    theme: &DiagramTheme,
    engine: MermaidEngine,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_packet_source_with_theme_and_engine(packet_source(), theme, engine, diagram_id)
}

fn try_render_packet_source_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_packet_source_with_theme_requirement(
        source,
        theme,
        engine,
        diagram_id,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn try_render_packet_source_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    diagram_id: &str,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Packet")
        .expect("detect themed Packet")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Packet session");

    family::prepare(&parsed, &LayoutOptions::default(), session)?.render_svg(
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
fn packet_block_paint_uses_real_rectangles_and_property_local_config_ownership() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#111827").unwrap())
                        .with_stroke(CanvasPaint::solid("#f97316").unwrap()),
                )),
            ),
        )
        .unwrap();
    for (config, expected_fill) in [
        (serde_json::json!({}), "#111827"),
        (
            serde_json::json!({"themeVariables": {"packet": {"blockFillColor": "#fef3c7"}}}),
            "#fef3c7",
        ),
        (
            serde_json::json!({"themeVariables": {"packet": {"blockFillColor": true}}}),
            "#111827",
        ),
    ] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(config),
        );
        let rendered = render_packet_with_theme_and_engine(&theme, engine, "packet-block-paint");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("class") == Some("packetBlock"))
        );
        assert!(packet_stylesheet(&document).contains(&format!(
            ".packetBlock{{stroke:#f97316;stroke-width:1;fill:{expected_fill};}}",
        )));
        drop(document);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn packet_block_paint_keeps_unsupported_geometry_as_a_residual() {
    let mut patch = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#111827").unwrap());
    patch.geometry.radius = Specified::Value(4.0);
    let styles = ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch));
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .unwrap();
    let error = try_render_packet_source_with_theme_and_engine(
        packet_source(),
        &theme,
        MermaidEngine::new(),
        "packet-mixed-block",
    )
    .err()
    .expect("a real unsupported block radius must retain its residual");
    assert!(error.unverified_family_theme().is_some());
    assert!(error.incomplete_family_theme().is_none());
}

#[test]
fn packet_block_static_receipts_require_real_occurrence_winners() {
    for stroke in [false, true] {
        let patch = |color| {
            let paint = CanvasPaint::solid(color).unwrap();
            if stroke {
                ThemeStylePatch::default().with_stroke(paint)
            } else {
                ThemeStylePatch::default().with_fill(paint)
            }
        };
        for (source, count) in [
            ("packet\n0-7: \"Header\"\n", 1),
            ("packet\n0-7: \"Header\"\n8-15: \"Payload\"\n", 2),
        ] {
            for (ordinal, static_last) in [(1, false), (1, true), (999, false)] {
                let mut rules = [
                    ThemeRule::new(ThemeTarget::Node, patch("#111111")),
                    ThemeRule::new(ThemeTarget::Node, patch("#222222"))
                        .with_ordinal(OrdinalSelector::exact(ordinal).unwrap()),
                ];
                if static_last {
                    rules.reverse();
                }
                let styles = rules
                    .into_iter()
                    .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
                let theme = DiagramThemeCompiler::new()
                    .compile(DiagramThemeSpec::new().with_styles(styles))
                    .unwrap();
                let rendered = try_render_packet_source_with_theme_requirement(
                    source,
                    &theme,
                    MermaidEngine::new(),
                    "packet-block-occurrence-winners",
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert_eq!(
                    document
                        .descendants()
                        .filter(|node| node.attribute("class") == Some("packetBlock"))
                        .count(),
                    count
                );
                let expected = if stroke {
                    ".packetBlock{stroke:#111111;stroke-width:1;fill:#efefef;}"
                } else {
                    ".packetBlock{stroke:black;stroke-width:1;fill:#111111;}"
                };
                assert!(packet_stylesheet(&document).contains(expected));
                drop(document);
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                let residual = !static_last && ordinal <= count;
                let applied = !residual || count > 1;
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(evidence.accounted_count(), 2);
                assert_eq!(evidence.applied_count(), usize::from(applied));
                assert_eq!(
                    evidence.not_applicable_count(),
                    usize::from(!residual) + usize::from(!applied)
                );
                assert_eq!(evidence.theme_residual_count(), usize::from(residual));
                let strict = try_render_packet_source_with_theme_and_engine(
                    source,
                    &theme,
                    MermaidEngine::new(),
                    "packet-block-occurrence-winners",
                );
                if residual {
                    let error = strict
                        .err()
                        .expect("a winning ordinal block paint remains unsupported");
                    assert_eq!(
                        error.unverified_family_theme(),
                        Some((DiagramFamilyId::PACKET, 1))
                    );
                    assert_eq!(error.incomplete_family_theme(), None);
                } else {
                    strict.expect("a shadowed or absent ordinal request is portable");
                }
            }
        }
    }
}

#[test]
fn packet_block_source_ownership_preserves_independent_paint_and_geometry_evidence() {
    for (source_owns_stroke, radius) in [(false, false), (true, false), (true, true)] {
        let mut patch = ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#111111").unwrap())
            .with_stroke(CanvasPaint::solid("#444444").unwrap());
        if radius {
            patch.geometry.radius = Specified::Value(4.0);
        }
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(ThemeTarget::Node, patch))
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Node,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#222222").unwrap()),
                            )
                            .with_ordinal(OrdinalSelector::exact(1).unwrap()),
                        ),
                ),
            )
            .unwrap();
        let config = if source_owns_stroke {
            serde_json::json!({"themeVariables": {"packet": {"blockStrokeColor": "#abcdef"}}})
        } else {
            serde_json::json!({})
        };
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(config),
        );
        let rendered = try_render_packet_source_with_theme_requirement(
            "packet\n0-7: \"Header\"\n",
            &theme,
            engine,
            "packet-block-property-evidence",
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let border = if source_owns_stroke {
            "#abcdef"
        } else {
            "#444444"
        };
        assert!(packet_stylesheet(&document).contains(&format!(
            ".packetBlock{{stroke:{border};stroke-width:1;fill:#111111;}}"
        )));
        drop(document);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.accounted_count(), 2);
        assert_eq!(evidence.applied_count(), usize::from(!source_owns_stroke));
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(source_owns_stroke && !radius)
        );
        assert_eq!(evidence.theme_residual_count(), 1 + usize::from(radius));
    }
}

#[test]
fn packet_empty_theme_preserves_native_block_svg() {
    let source = "packet\n0-7: \"Header\"\n";
    let (_, baseline) = render_packet(source);
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert_eq!(rendered.svg().as_bytes(), baseline.as_bytes());
}

#[test]
fn packet_default_and_dark_themes_reach_every_visible_css_role() {
    for (theme, text_color, block_fill) in
        [("default", "black", "#efefef"), ("dark", "#e0dfdf", "#333")]
    {
        for owner in ["source", "site"] {
            let config = serde_json::json!({"theme": theme});
            let (source, site_config) = if owner == "source" {
                (
                    format!("%%{{init: {config}}}%%\n{}", packet_source()),
                    serde_json::json!({"secure": []}),
                )
            } else {
                (packet_source().to_owned(), config)
            };
            let (_, svg) = render_packet_with_engine(
                &source,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site_config),
                ),
            );
            let document = roxmltree::Document::parse(&svg).expect("valid Packet theme SVG");
            let css = packet_stylesheet(&document);

            for (class, rule) in [
                (
                    "packetByte start",
                    format!(".packetByte.start{{fill:{text_color};}}"),
                ),
                (
                    "packetByte end",
                    format!(".packetByte.end{{fill:{text_color};}}"),
                ),
                (
                    "packetLabel",
                    format!(".packetLabel{{fill:{text_color};font-size:12px;}}"),
                ),
                (
                    "packetTitle",
                    format!(".packetTitle{{fill:{text_color};font-size:14px;}}"),
                ),
                (
                    "packetBlock",
                    format!(
                        ".packetBlock{{stroke:{text_color};stroke-width:1;fill:{block_fill};}}"
                    ),
                ),
            ] {
                assert!(
                    document
                        .descendants()
                        .any(|node| node.attribute("class") == Some(class)),
                    "{owner}/{theme}: missing visible {class} consumer"
                );
                assert!(
                    css.contains(&rule),
                    "{owner}/{theme}: expected {rule}: {css}"
                );
            }
            assert!(css.contains(".packetByte{font-size:10px;}"));
        }
    }
}

#[test]
fn packet_source_palette_owns_present_text_roles_without_owning_typed_fonts() {
    let theme = packet_direct_text_theme_with_mermaid(MermaidThemeCompatibility::default());
    for palette in [
        "base",
        "default",
        "dark",
        "forest",
        "neutral",
        "neo",
        "neo-dark",
        "redux",
        "redux-dark",
        "redux-color",
        "redux-dark-color",
    ] {
        for owner in ["source", "site"] {
            let config = serde_json::json!({"theme": palette});
            let (source, site_config) = if owner == "source" {
                (
                    format!("%%{{init: {config}}}%%\n{}", packet_source()),
                    serde_json::json!({}),
                )
            } else {
                (packet_source().to_owned(), config)
            };
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(site_config),
            );
            let rendered = try_render_packet_source_with_theme_and_engine(
                &source,
                &theme,
                engine,
                "packet-palette-text-ownership",
            )
            .expect("Packet paint ownership must preserve independent typed font evidence");
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let css = packet_stylesheet(&document);
            let palette_text = match (owner, palette) {
                ("source", "dark") => Some("#e0dfdf"),
                ("source", "forest") => Some("#321b67"),
                _ => None,
            };
            for (class, rule) in [
                (
                    "packetByte start",
                    format!(
                        ".packetByte.start{{fill:{};}}",
                        palette_text.unwrap_or("#e5e7eb")
                    ),
                ),
                (
                    "packetByte end",
                    format!(
                        ".packetByte.end{{fill:{};}}",
                        palette_text.unwrap_or("#e5e7eb")
                    ),
                ),
                (
                    "packetLabel",
                    format!(
                        ".packetLabel{{fill:{};font-size:12px;}}",
                        palette_text.unwrap_or("#111827")
                    ),
                ),
                (
                    "packetTitle",
                    format!(
                        ".packetTitle{{fill:{};font-size:14px;}}",
                        palette_text.unwrap_or("#fef3c7")
                    ),
                ),
            ] {
                assert!(
                    document
                        .descendants()
                        .any(|node| node.attribute("class") == Some(class)),
                    "{owner}/{palette}: missing visible {class}"
                );
                assert!(css.contains(&rule), "{owner}/{palette}: {css}");
            }
            assert!(
                css.contains("#packet-palette-text-ownership{font-family:monospace;"),
                "{owner}/{palette}: palette must not own the font stack"
            );
            drop(document);
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let owned_roles = if palette_text.is_some() { 3 } else { 0 };
            assert_eq!(evidence.required_count(), 4, "{owner}/{palette}");
            assert_eq!(evidence.accounted_count(), 4, "{owner}/{palette}");
            assert_eq!(
                evidence.applied_count(),
                4 - owned_roles,
                "{owner}/{palette}"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                owned_roles,
                "{owner}/{palette}"
            );
            assert_eq!(evidence.theme_residual_count(), 0, "{owner}/{palette}");
        }
    }
}

#[test]
fn packet_source_palette_owns_generic_text_field_label_fallback() {
    let theme = packet_text_fill_theme(CanvasPaint::solid("#123456").unwrap());
    for (palette, expected_color) in [("dark", "#e0dfdf"), ("forest", "#321b67")] {
        let rendered = try_render_packet_source_with_theme_and_engine(
            &format!(
                "%%{{init: {{\"theme\": \"{palette}\"}}}}%%\n{}",
                packet_source()
            ),
            &theme,
            MermaidEngine::new(),
            "packet-palette-generic-text",
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(packet_stylesheet(&document).contains(&format!(
            ".packetLabel{{fill:{expected_color};font-size:12px;}}"
        )));
        assert!(document.descendants().any(|node| {
            node.attribute("class") == Some("packetLabel") && node.text() == Some("Header")
        }));
        drop(document);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{palette}");
        assert_eq!(evidence.accounted_count(), 1, "{palette}");
        assert_eq!(evidence.applied_count(), 0, "{palette}");
        assert_eq!(evidence.not_applicable_count(), 1, "{palette}");
        assert_eq!(evidence.theme_residual_count(), 0, "{palette}");
    }
}

#[test]
fn packet_theme_variable_styles_preserve_root_layout_configuration() {
    for owner in ["source", "site"] {
        let config = serde_json::json!({
            "packet": {
                "showBits": false,
                "rowHeight": 40,
                "labelColor": "#010203",
                "blockFillColor": "#040506"
            },
            "themeVariables": { "packet": {
                "showBits": true,
                "rowHeight": 80,
                "byteFontSize": "11px",
                "startByteColor": "#111111",
                "endByteColor": "#222222",
                "labelColor": "#333333",
                "labelFontSize": "13px",
                "titleColor": "#444444",
                "titleFontSize": "15px",
                "blockStrokeColor": "#555555",
                "blockStrokeWidth": 2,
                "blockFillColor": "#666666"
            } }
        });
        let (source, site_config) = if owner == "source" {
            (
                format!("%%{{init: {config}}}%%\n{}", packet_source()),
                serde_json::json!({"secure": []}),
            )
        } else {
            (packet_source().to_owned(), config)
        };
        let (layout, svg) = render_packet_with_engine(
            &source,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(site_config),
            ),
        );
        assert!(
            !layout.show_bits,
            "{owner}: layout must still read root packet.showBits"
        );
        assert_eq!(layout.row_height, 40.0, "{owner}");
        let document = roxmltree::Document::parse(&svg).expect("valid configured Packet SVG");
        let css = packet_stylesheet(&document);
        // Init directives retain Mermaid's key allowlist; site config can supply every role.
        let (byte_size, start, end, block) = if owner == "source" {
            (
                "10px",
                "black",
                "black",
                "stroke:black;stroke-width:1;fill:#efefef;",
            )
        } else {
            (
                "11px",
                "#111111",
                "#222222",
                "stroke:#555555;stroke-width:2;fill:#666666;",
            )
        };
        for rule in [
            format!(".packetByte{{font-size:{byte_size};}}"),
            format!(".packetByte.start{{fill:{start};}}"),
            format!(".packetByte.end{{fill:{end};}}"),
            ".packetLabel{fill:#333333;font-size:13px;}".into(),
            ".packetTitle{fill:#444444;font-size:15px;}".into(),
            format!(".packetBlock{{{block}}}"),
        ] {
            assert!(css.contains(&rule), "{owner}: expected {rule}: {css}");
        }
        assert!(
            !document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|class| class.starts_with("packetByte "))
            }),
            "{owner}: root showBits=false must hide byte text"
        );
    }
}

#[test]
fn packet_byte_label_and_title_fill_reach_only_their_terminal_roles() {
    let theme = packet_byte_label_and_title_fill_theme(
        CanvasPaint::solid("#e5e7eb").expect("valid Packet byte-label fill"),
        CanvasPaint::solid("#fef3c7").expect("valid Packet title fill"),
    );
    let rendered = render_packet_with_theme_and_engine(
        &theme,
        MermaidEngine::new(),
        "packet-terminal-text-fill",
    );
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
    for owner in ["source", "site"] {
        let config = serde_json::json!({
            "themeVariables": { "packet": {
                "startByteColor": "#111111",
                "labelColor": "#222222",
                "titleColor": "#333333"
            } }
        });
        let (source, site_config) = if owner == "source" {
            (
                format!("%%{{init: {config}}}%%\n{}", packet_source()),
                serde_json::json!({"secure": []}),
            )
        } else {
            (packet_source().to_owned(), config)
        };
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(site_config),
        );
        let rendered = try_render_packet_source_with_theme_and_engine(
            &source,
            &theme,
            engine,
            "packet-explicit-role-colors",
        )
        .expect("explicit Packet theme variables must own only their text roles");
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid configured Packet SVG");
        let css = packet_stylesheet(&document);

        let byte_start = if owner == "source" {
            "#e5e7eb"
        } else {
            "#111111"
        };
        for rule in [
            format!(".packetByte.start{{fill:{byte_start};}}"),
            ".packetByte.end{fill:#e5e7eb;}".into(),
            ".packetLabel{fill:#222222;font-size:12px;}".into(),
            ".packetTitle{fill:#333333;font-size:14px;}".into(),
        ] {
            assert!(css.contains(&rule), "{owner}: expected {rule}: {css}");
        }

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2, "{owner}");
        assert_eq!(evidence.accounted_count(), 2, "{owner}");
        assert_eq!(evidence.applied_count(), 1, "{owner}");
        assert_eq!(evidence.not_applicable_count(), 1, "{owner}");
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
    }
}

#[test]
fn packet_dark_presets_pair_each_text_role_with_its_terminal_background() {
    let cases = [
        (ThemePreset::EditorDark, "#e5e7eb", "#111827", "#475569"),
        (ThemePreset::OneDark, "#abb2bf", "#21252b", "#3e4451"),
        (ThemePreset::GruvboxDark, "#ebdbb2", "#3c3836", "#665c54"),
        (ThemePreset::AyuDark, "#bfbdb6", "#11151c", "#343b48"),
    ];

    for (preset, expected_text_fill, surface, border) in cases {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset)
            .expect("compile dark Packet preset");
        let metadata = common::install_parse_compatibility(&theme, MermaidEngine::new())
            .parse_metadata_sync(packet_source())
            .expect("parse dark Packet preset metadata");
        let parse_evidence = metadata.effective_config.theme_compatibility().clone();
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
        let rendered =
            render_packet_with_theme_and_engine(&theme, MermaidEngine::new(), &diagram_id);
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid preset Packet SVG");
        let css = packet_stylesheet(&document);

        for expected_rule in [
            format!("#{diagram_id} .packetByte.start{{fill:{expected_text_fill};}}"),
            format!("#{diagram_id} .packetByte.end{{fill:{expected_text_fill};}}"),
            format!("#{diagram_id} .packetLabel{{fill:{expected_text_fill};font-size:12px;}}"),
            format!("#{diagram_id} .packetTitle{{fill:{expected_text_fill};font-size:14px;}}"),
        ] {
            assert!(
                css.contains(&expected_rule),
                "{preset:?} must brighten Packet text on its dark canvas and blocks: {css}"
            );
        }
        assert!(
            css.contains(&format!(
                "#{diagram_id} .packetBlock{{stroke:{border};stroke-width:1;fill:{surface};}}"
            )),
            "{preset:?} must use its typed Packet block surface and border: {css}"
        );

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 8);
        assert_eq!(evidence.accounted_count(), 8);
        assert_eq!(evidence.applied_count(), 6);
        assert_eq!(evidence.not_applicable_count(), 2);
        assert_eq!(evidence.theme_residual_count(), 0);
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
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(site_config),
        );
        let metadata = common::install_parse_compatibility(&theme, engine.clone())
            .parse_metadata_sync(packet_source())
            .expect("parse one-sided Packet dark-mode ownership");
        let parse_evidence = metadata.effective_config.theme_compatibility().clone();

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
        MermaidEngine::new(),
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "fontFamily": "Courier, monospace"
        })),
    );
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
        MermaidEngine::new(),
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "darkMode": false
        })),
    );
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
        MermaidEngine::new(),
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
        MermaidEngine::new(),
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
fn packet_zero_sized_field_labels_do_not_certify_text_fill_or_font_stack() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::PACKET,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("monospace").unwrap()),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
    // Source presentation accepts CSS values; declaration priority is a host capability.
    for (owner, size, expected_css) in [
        ("site", serde_json::json!(0), "0"),
        ("source", serde_json::json!(0), "0"),
        ("site", serde_json::json!("0px"), "0px"),
        ("source", serde_json::json!("0px"), "0px"),
        ("source", serde_json::json!("0em"), "0em"),
        ("source", serde_json::json!("0rem"), "0rem"),
        ("source", serde_json::json!("0%"), "0%"),
        ("source", serde_json::json!("0pt"), "0pt"),
        (
            "site",
            serde_json::json!("0.0px !important"),
            "0.0px !important",
        ),
    ] {
        let config = serde_json::json!({
            "packet": {"showBits": false},
            "themeVariables": {"packet": {"labelFontSize": size}}
        });
        let body = "packet\n0-7: \"Header\"\n";
        let (source, engine) = if owner == "source" {
            (
                format!("---\nconfig: {config}\n---\n{body}"),
                MermaidEngine::new(),
            )
        } else {
            (
                body.to_owned(),
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config),
                ),
            )
        };
        let rendered = try_render_packet_source_with_theme_and_engine(
            &source,
            &theme,
            engine,
            "packet-zero-sized-text",
        )
        .expect("zero-sized Packet labels are not applicable to text fill or font evidence");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            packet_stylesheet(&document).contains(&format!(
                ".packetLabel{{fill:#123456;font-size:{expected_css};}}"
            )),
            "{owner}/{size}: {}",
            packet_stylesheet(&document)
        );
        assert!(document.descendants().any(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("packetLabel")
                && node.text() == Some("Header")
        }));
        assert!(document.descendants().any(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("packetBlock")
        }));
        drop(document);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2, "{owner}/{size}");
        assert_eq!(evidence.accounted_count(), 2, "{owner}/{size}");
        assert_eq!(evidence.applied_count(), 0, "{owner}/{size}");
        assert_eq!(evidence.not_applicable_count(), 2, "{owner}/{size}");
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}/{size}");
    }
}

#[test]
fn packet_text_evidence_tracks_each_final_role_font_size() {
    let theme = packet_direct_text_theme_with_mermaid(MermaidThemeCompatibility::default());
    for visible_role in [None, Some("byte"), Some("label"), Some("title")] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": {"packet": {
                    "byteFontSize": if visible_role == Some("byte") {"10px"} else {"0px"},
                    "labelFontSize": if visible_role == Some("label") {"12px"} else {"0px"},
                    "titleFontSize": if visible_role == Some("title") {"14px"} else {"0px"}
                }}
            })),
        );
        let rendered =
            try_render_packet_with_theme_and_engine(&theme, engine, "packet-role-font-size")
                .expect(
                    "only the visible Packet role consumes its fill and the inherited font stack",
                );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        let applied = if visible_role.is_some() { 2 } else { 0 };
        assert_eq!(evidence.required_count(), 4, "{visible_role:?}");
        assert_eq!(evidence.accounted_count(), 4, "{visible_role:?}");
        assert_eq!(evidence.applied_count(), applied, "{visible_role:?}");
        assert_eq!(
            evidence.not_applicable_count(),
            4 - applied,
            "{visible_role:?}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{visible_role:?}");
    }
}

#[test]
fn packet_zero_sized_text_keeps_geometry_residuals_but_retires_fill_winners() {
    for target in [ThemeTarget::Text, ThemeTarget::PacketFieldLabel] {
        for (ordinal, clear) in [(false, false), (true, false), (false, true)] {
            for geometry in [false, true] {
                let mut patch =
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
                if clear {
                    patch.paint.fill = Specified::Clear;
                }
                if geometry {
                    patch.geometry.radius = Specified::Value(8.0);
                }
                let mut rule = ThemeRule::new(target, patch);
                if ordinal {
                    rule = rule.with_ordinal(OrdinalSelector::exact(1).unwrap());
                }
                let theme = DiagramThemeCompiler::new()
                    .compile(
                        DiagramThemeSpec::new()
                            .with_styles(ThemeRuleSet::default().with_rule(rule)),
                    )
                    .unwrap();
                let engine = MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({
                            "packet": {"showBits": false},
                            "themeVariables": {"packet": {"labelFontSize": "0px"}}
                        }),
                    ),
                );
                let rendered = try_render_packet_source_with_theme_requirement(
                    "packet\n0-7: \"Header\"\n",
                    &theme,
                    engine,
                    "packet-zero-sized-text-geometry",
                    if geometry {
                        ThemePortabilityRequirement::BestEffort
                    } else {
                        ThemePortabilityRequirement::RequirePortable
                    },
                )
                .expect("zero-sized text retains its geometry occurrence domain");
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                let case =
                    format!("{target:?}/ordinal={ordinal}/clear={clear}/geometry={geometry}");
                assert_eq!(evidence.required_count(), 1, "{case}");
                assert_eq!(evidence.accounted_count(), 1, "{case}");
                assert_eq!(evidence.applied_count(), 0, "{case}");
                assert_eq!(
                    evidence.not_applicable_count(),
                    usize::from(!geometry),
                    "{case}"
                );
                assert_eq!(
                    evidence.theme_residual_count(),
                    usize::from(geometry),
                    "{case}"
                );
            }
        }
    }
}

#[test]
fn packet_source_label_color_outranks_generic_text_fill_in_styles_and_report() {
    let theme = packet_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Packet generic text fill"),
    );
    let rendered = try_render_packet_source_with_theme_and_engine(
        r##"%%{init: {"themeVariables": {"packet": {"labelColor": "#abcdef"}}}}%%
packet
0-7: "Header"
"##,
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "secure": []
            })),
        ),
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
fn packet_site_label_color_outranks_generic_text_fill_in_styles_and_report() {
    let theme = packet_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Packet generic text fill"),
    );
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "packet": {
                "labelColor": "#abcdef"
            } }
        })),
    );
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
fn packet_ignored_role_colors_do_not_claim_typed_fill_ownership() {
    let theme = packet_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid Packet generic text fill"),
    );
    for (case, source, site_config) in [
        (
            "root-site",
            "packet\n0-7: \"Header\"\n",
            serde_json::json!({"packet": {"labelColor": "#abcdef"}}),
        ),
        (
            "root-source",
            "%%{init: {\"packet\": {\"labelColor\": \"#abcdef\"}}}%%\npacket\n0-7: \"Header\"\n",
            serde_json::json!({"secure": []}),
        ),
        (
            "secure-source",
            "%%{init: {\"themeVariables\": {\"packet\": {\"labelColor\": \"#abcdef\"}}}}%%\npacket\n0-7: \"Header\"\n",
            serde_json::json!({"secure": ["themeVariables"]}),
        ),
        (
            "invalid-site-type",
            "packet\n0-7: \"Header\"\n",
            serde_json::json!({"themeVariables": {"packet": {"labelColor": true}}}),
        ),
        (
            "invalid-source-type",
            "%%{init: {\"themeVariables\": {\"packet\": {\"labelColor\": true}}}}%%\npacket\n0-7: \"Header\"\n",
            serde_json::json!({"secure": []}),
        ),
        (
            "unsafe-site-value",
            "packet\n0-7: \"Header\"\n",
            serde_json::json!({"themeVariables": {"packet": {"labelColor": "red;font-family:serif"}}}),
        ),
    ] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(site_config),
        );
        let rendered = try_render_packet_source_with_theme_and_engine(
            source,
            &theme,
            engine,
            "packet-ignored-role-color",
        )
        .expect("ignored Packet colors must leave typed fill active");
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Packet SVG");
        let css = packet_stylesheet(&document);
        assert!(
            css.contains(".packetLabel{fill:#123456;font-size:12px;}"),
            "{case}: ignored colors must not own terminal Packet CSS: {css}"
        );

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{case}");
        assert_eq!(evidence.accounted_count(), 1, "{case}");
        assert_eq!(evidence.applied_count(), 1, "{case}");
        assert_eq!(evidence.not_applicable_count(), 0, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn packet_byte_label_ordinal_counts_each_visible_byte_occurrence() {
    let theme = packet_ordinal_fill_theme(ThemeTarget::PacketByteLabel, 2);
    let error = try_render_packet_with_theme_and_engine(
        &theme,
        MermaidEngine::new(),
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
        MermaidEngine::new(),
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
        MermaidEngine::new(),
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
                    r##"%%{init: {"themeVariables": {"packet": {"labelColor": "#abcdef"}}}}%%
packet
0-7: "Header"
"##,
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            serde_json::json!({"secure": []}),
                        ),
                    ),
                )
            } else {
                (
                    "packet\n0-7: \"Header\"\n",
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            serde_json::json!({
                                "themeVariables": { "packet": { "labelColor": "#abcdef" } }
                            }),
                        ),
                    ),
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "packet": { "labelColor": "#abcdef" } }
        })),
    );
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
            MermaidEngine::new(),
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
    let rendered =
        render_packet_with_theme_and_engine(&theme, MermaidEngine::new(), "packet-theme");
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
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(site_config),
        );
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
        assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
    }
}

#[test]
fn packet_invalid_style_values_fall_back_without_overriding_the_typed_font_stack() {
    let theme = packet_font_theme("monospace");
    for owner in ["source", "site"] {
        let mut config = serde_json::json!({"themeVariables": {"packet": {}}});
        for property in [
            "byteFontSize",
            "startByteColor",
            "endByteColor",
            "labelColor",
            "labelFontSize",
            "titleColor",
            "titleFontSize",
            "blockStrokeColor",
            "blockStrokeWidth",
            "blockFillColor",
        ] {
            config["themeVariables"]["packet"][property] =
                serde_json::json!("red;font-family:serif");
        }
        let (source, site_config) = if owner == "source" {
            (
                format!("%%{{init: {config}}}%%\n{}", packet_source()),
                serde_json::json!({"secure": []}),
            )
        } else {
            (packet_source().to_owned(), config)
        };
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(site_config),
        );
        let rendered = try_render_packet_source_with_theme_and_engine(
            &source,
            &theme,
            engine,
            "packet-safe-role-style",
        )
        .expect("invalid Packet values must preserve defaults and typed typography");
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid safely configured Packet SVG");
        let css = packet_stylesheet(&document);

        for rule in [
            "#packet-safe-role-style{font-family:monospace;font-size:16px;fill:#333;}",
            ".packetByte{font-size:10px;}",
            ".packetByte.start{fill:black;}",
            ".packetByte.end{fill:black;}",
            ".packetLabel{fill:black;font-size:12px;}",
            ".packetTitle{fill:black;font-size:14px;}",
            ".packetBlock{stroke:black;stroke-width:1;fill:#efefef;}",
        ] {
            assert!(
                css.contains(rule),
                "{owner}: expected safe fallback {rule}: {css}"
            );
        }
        assert!(!css.contains("font-family:serif"), "{owner}");

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{owner}");
        assert_eq!(evidence.accounted_count(), 1, "{owner}");
        assert_eq!(evidence.applied_count(), 1, "{owner}");
        assert_eq!(evidence.not_applicable_count(), 0, "{owner}");
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
    }
}

#[test]
fn packet_large_direct_font_stack_reaches_styles_and_report() {
    let families = (0..32)
        .map(|index| format!("PacketFont{index:02}{}", "x".repeat(140)))
        .collect::<Vec<_>>();
    let font_stack = FontStack::new(families).expect("valid maximum-width Packet font stack");
    let expected_css = font_stack.as_css();
    assert!(expected_css.len() > 4 * 1024);

    let theme = packet_font_stack_theme(font_stack);
    let rendered = render_packet_with_theme_and_engine(
        &theme,
        MermaidEngine::new(),
        "packet-large-font-stack",
    );
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
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

const DESCENDING_PACKET: &str = include_str!("../../../fixtures/packet/bit_order_descending.mmd");

fn render_packet(source: &str) -> (PacketDiagramLayout, String) {
    render_packet_with_engine(source, MermaidEngine::new())
}

fn render_packet_with_engine(source: &str, engine: MermaidEngine) -> (PacketDiagramLayout, String) {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse packet")
        .expect("packet diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::default(), session).expect("packet layout");
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
fn packet_explicit_root_font_size_preserves_relative_roles_and_fixed_geometry() {
    let body = "packet\n0-7: \"Header\"\n";
    let (baseline, _) = render_packet(&format!(
        "---\nconfig:\n  packet:\n    showBits: false\n---\n{body}"
    ));
    for (root_size, expected_px) in [
        (serde_json::json!("32px"), "32"),
        (serde_json::json!("24pt"), "32"),
        (serde_json::json!("0px"), "0"),
        (serde_json::json!("0.5px"), "0.5"),
        (serde_json::json!("0.0000000001px"), "0.0000000001"),
    ] {
        let config = serde_json::json!({
            "packet": {"showBits": false},
            "themeVariables": {
                "fontFamily": "monospace",
                "fontSize": root_size,
                "packet": {"labelFontSize": "1em"}
            }
        });
        let (layout, svg) = render_packet(&format!("---\nconfig: {config}\n---\n{body}"));
        assert_eq!(
            serde_json::to_value(&layout).unwrap(),
            serde_json::to_value(&baseline).unwrap(),
            "root size {root_size} must not change Packet geometry",
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let css = packet_stylesheet(&document);
        for selector in ["#merman", "#merman svg"] {
            assert!(
                css.contains(&format!(
                    "{selector}{{font-family:monospace;font-size:{expected_px}px;"
                )),
                "{root_size}: {css}",
            );
        }
        assert!(css.contains(".packetLabel{fill:black;font-size:1em;}"));
        assert!(css.contains(".packetByte{font-size:10px;}"));
        assert!(css.contains(".packetTitle{fill:black;font-size:14px;}"));
    }
}

#[test]
fn packet_zero_root_font_size_accounts_for_relative_and_absolute_roles() {
    use serde_json::json;

    let theme = packet_direct_text_theme_with_mermaid(MermaidThemeCompatibility::default());
    for (label_size, applied) in [
        (json!("1em"), 0),
        (json!("100%"), 0),
        (json!("inherit"), 0),
        (json!("unset"), 0),
        (json!("-1em"), 0),
        (json!("-1px"), 0),
        (json!(24), 0),
        (json!("24"), 0),
        (json!("1ex"), 0),
        (json!("1ch"), 0),
        (json!("1rem"), 2),
        (json!("12px"), 2),
    ] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "packet": {"showBits": false},
                "themeVariables": {
                    "fontSize": "0px",
                    "packet": {"labelFontSize": label_size}
                }
            })),
        );
        let rendered = try_render_packet_source_with_theme_and_engine(
            "packet\n0-7: \"Header\"\n",
            &theme,
            engine,
            "packet-zero-parent",
        )
        .expect("relative zero-sized Packet text must not consume paint or fonts");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 4, "{label_size}");
        assert_eq!(evidence.applied_count(), applied, "{label_size}");
        assert_eq!(evidence.not_applicable_count(), 4 - applied, "{label_size}");
        assert_eq!(evidence.theme_residual_count(), 0, "{label_size}");
    }
}

#[test]
fn packet_multiline_labels_reach_svg_as_literal_text() {
    let source = "packet-beta\n0: \"雪\n<node>&%%\"\n1: \"Tail\"\n";
    for source in [source.to_owned(), source.replace('\n', "\r\n")] {
        let (layout, svg) = render_packet(&source);
        assert_eq!(layout.words[0].blocks.len(), 2);
        let document = roxmltree::Document::parse(&svg).expect("valid literal Packet SVG");
        let labels = document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetLabel"))
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(labels, ["雪\n<node>&%%", "Tail"]);
        assert!(!document.descendants().any(|node| node.has_tag_name("node")));
    }
}

#[test]
fn packet_intrinsic_svg_size_follows_effective_family_configuration() {
    use merman_core::presentation::SourcePresentationConfig;

    for (site_value, source_value, responsive) in [
        (None, None, true),
        (Some(false), None, false),
        (Some(true), Some(false), false),
        (Some(false), Some(true), true),
    ] {
        let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
            serde_json::json!({"useMaxWidth": false, "packet": {"useMaxWidth": site_value}}),
        ));
        let prefix = source_value.map_or_else(String::new, |value| {
            format!("---\nconfig:\n  packet:\n    useMaxWidth: {value}\n---\n")
        });
        let (_, svg) = render_packet_with_engine(
            &format!("{prefix}packet-beta\n0-31: \"Header\"\n+8: \"Tail\"\n"),
            engine,
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let root = document.root_element();
        let view_box = root
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        if responsive {
            assert_eq!(root.attribute("width"), Some("100%"));
            assert!(root.attribute("height").is_none());
            assert!(root.attribute("style").unwrap().contains("max-width:"));
        } else {
            assert_eq!(
                root.attribute("width").unwrap().parse::<f64>().unwrap(),
                view_box[2]
            );
            assert_eq!(
                root.attribute("height").unwrap().parse::<f64>().unwrap(),
                view_box[3]
            );
            assert!(root.attribute("style").is_none());
        }
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.attribute("class") == Some("packetBlock"))
                .count(),
            2
        );
    }
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

#[allow(dead_code)]
mod common;
